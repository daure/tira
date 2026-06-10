use super::{App, AppEffect, ApplicationTab, JiraLoadPurpose, ListView, tree_items_from_issues};
use crate::components::generic::notification::Notification;
use crate::components::generic::tree::TreeItem;
use crate::services::jira::{CommandLogEntry, IssueSummary, JiraError, JiraLoadResult};

/// Rows of look-ahead kept loaded below the revealed bottom of the list. The
/// next root page is pulled in once the user scrolls within this many rows of
/// the end, so a large project pages in lazily instead of eagerly up front.
const ROOT_PREFETCH_LOOKAHEAD: usize = 30;

/// Characters required in the search box before a server search fires. Below
/// this the list falls back to the browse view rather than searching for one or
/// two stray characters.
const SEARCH_MIN_CHARS: usize = 2;

/// Idle time after the last keystroke before the debounced search fires, so
/// typing "DPP-1234" issues one query instead of one per character.
const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(300);

impl App {
    /// Queues the next page of root issues if one is pending and no root-page
    /// load is already in flight. Browsing only. Fetches up to
    /// `ROOT_PAGE_SIZE` more roots.
    fn maybe_queue_next_root_page(&mut self) {
        self.queue_next_root_page(crate::services::jira::ROOT_PAGE_SIZE);
    }

    /// Queues a root-page fetch of up to `max_results` more roots from the
    /// current page token. Used both for lazy single-page paging and to pull a
    /// whole already-loaded extent back in one query on reload. Gated on browse
    /// view, a pending token, and no in-flight page.
    fn queue_next_root_page(&mut self, max_results: u32) {
        if self.pending_roots_request_id.is_some() || self.view != ListView::Browse {
            return;
        }
        let Some(token) = self.next_root_page_token.clone() else {
            return;
        };
        let Some(credentials) = self.credentials.clone() else {
            return;
        };
        let request_id = self.next_request_id();
        self.pending_roots_request_id = Some(request_id);
        // Lazy browse paging (not part of a full reload, which has its own
        // status): show progress in the footer while the next page loads.
        if self.reload_root_ids.is_none() {
            self.status = String::from("Loading more issues…");
        }
        self.pending_effects.push(AppEffect::LoadMoreRoots {
            request_id,
            credentials,
            fields: self.current_fields_param(),
            page_token: token,
            max_results,
        });
    }

    /// Pulls the next page of root issues when browsing nears the bottom of
    /// what's currently loaded, so large projects page in on demand rather than
    /// eagerly walking the whole project at startup. `viewport_height` is the
    /// visible row count when known (mouse-wheel scrolling); pass 0 for
    /// keyboard navigation, where the selection index is the bottom signal.
    /// The actual fetch is gated by `maybe_queue_next_root_page` (browse view,
    /// a pending token, and no in-flight page).
    pub(crate) fn maybe_prefetch_more_roots(&mut self, viewport_height: usize) {
        if self.view != ListView::Browse || self.next_root_page_token.is_none() {
            return;
        }
        let total = self.filtered_tree.visible_rows().len();
        // The furthest-down row currently revealed, whether by moving the
        // selection (keyboard) or scrolling the viewport (mouse wheel).
        let viewport_bottom = self
            .filtered_tree
            .scroll_offset()
            .saturating_add(viewport_height);
        let revealed = self
            .filtered_tree
            .selected_item_index()
            .max(viewport_bottom);
        if revealed + ROOT_PREFETCH_LOOKAHEAD >= total {
            self.maybe_queue_next_root_page();
        }
    }

    /// Applies the first root page of a seamless (in-place) reload: merges the
    /// fresh roots without tearing the tree down, refreshes the rest of the
    /// previously-loaded extent in a single query (not page-by-page), and starts
    /// background refreshes of the previously-expanded subtrees. The selection,
    /// scroll, and expansion stay exactly where they were.
    pub(crate) fn begin_seamless_reload(
        &mut self,
        roots: Vec<TreeItem>,
        next_token: Option<String>,
    ) {
        // How far the user had paged before the reload: refresh exactly that
        // extent, in one query for the remainder, rather than eagerly walking
        // the whole project.
        let loaded_root_count = self.filtered_tree.root_count();
        let page_one_count = roots.len();
        let seen: std::collections::HashSet<String> =
            roots.iter().map(|item| item.id.clone()).collect();
        self.filtered_tree.merge_root_items(roots);
        self.next_root_page_token = next_token;

        let remainder = loaded_root_count.saturating_sub(page_one_count);
        if remainder > 0 {
            // More than the first page was loaded. Refresh the rest in one query
            // when a token is available, but NEVER prune the loaded roots — a
            // capped/short/absent remainder must not drop the user's extent or
            // scroll position. (Server-deleted roots beyond page 1 linger until a
            // fresh load; that's a far smaller cost than losing your place.)
            if self.next_root_page_token.is_some() {
                self.reload_root_ids = Some(std::collections::HashSet::new());
                let remainder =
                    remainder.min(crate::services::jira::CHILD_PAGE_SIZE as usize) as u32;
                self.queue_next_root_page(remainder);
            } else {
                self.reload_root_ids = None;
            }
        } else {
            // Page 1 covered the whole loaded extent: safe to prune any roots
            // deleted server-side.
            self.filtered_tree.retain_roots(&seen);
            self.reload_root_ids = None;
        }

        // The expanded subtrees were already greyed and refetched the instant
        // the reload started (see `reload_list`). Now that the roots have landed,
        // apply any child results that arrived first and were held back, so the
        // whole reload settles together as one unit.
        for (parent_key, issues) in std::mem::take(&mut self.reload_children_buffer) {
            self.soft_reload_parents.remove(&parent_key);
            self.settle_soft_reload(&parent_key, issues);
        }
    }

    pub(crate) fn apply_roots_page_result(&mut self, request_id: u64, result: JiraLoadResult) {
        self.command_log.extend(result.logs);
        if self.pending_roots_request_id != Some(request_id) {
            // Superseded by a newer load (reload, project switch, or search).
            return;
        }
        self.pending_roots_request_id = None;

        match result.issues {
            Ok(issues) => {
                let items = tree_items_from_issues(issues);
                if self.reload_root_ids.is_some() {
                    // The reload's remaining extent arrives in a single response:
                    // refresh it in place and hand back to lazy paging (the
                    // token, if any, is preserved). No prune — keep every loaded
                    // root and the user's scroll position even if this response
                    // is short or capped.
                    self.filtered_tree.merge_root_items(items);
                    self.next_root_page_token = result.next_page_token;
                    self.reload_root_ids = None;
                } else {
                    if self.view == ListView::Browse {
                        self.filtered_tree.append_items(items);
                        self.status = String::from("Jira issues loaded.");
                    }
                    self.next_root_page_token = result.next_page_token;
                }
            }
            Err(error) => {
                self.next_root_page_token = None;
                self.reload_root_ids = None;
                if self.view == ListView::Browse {
                    self.status = String::from("Jira issues loaded.");
                }
                self.notifications
                    .push(Notification::error("Could not load more issues", error.0));
            }
        }
    }

    pub(crate) fn request_children(&mut self, parent_key: String) {
        let Some(credentials) = self.credentials.clone() else {
            return;
        };
        let request_id = self.next_request_id();
        self.pending_child_requests
            .insert(parent_key.clone(), request_id);
        self.pending_effects.push(AppEffect::LoadChildren {
            request_id,
            credentials,
            parent_key,
            fields: self.current_fields_param(),
        });
    }

    /// Fetches the children of several parents in a single batched query rather
    /// than one request per parent. Each parent still gets its own request id
    /// (recorded in `pending_child_requests`), so the batched results flow back
    /// through the same per-parent handling as a single load. No-op when empty.
    pub(crate) fn request_children_batch(&mut self, parent_keys: Vec<String>) {
        if parent_keys.is_empty() {
            return;
        }
        let Some(credentials) = self.credentials.clone() else {
            return;
        };
        let parents = parent_keys
            .into_iter()
            .map(|parent_key| {
                let request_id = self.next_request_id();
                self.pending_child_requests
                    .insert(parent_key.clone(), request_id);
                (parent_key, request_id)
            })
            .collect();
        self.pending_effects.push(AppEffect::LoadChildrenBatch {
            credentials,
            parents,
            fields: self.current_fields_param(),
        });
    }

    /// Removes `parent_key` from the restore set once its children have arrived
    /// (or failed), so the restoration is considered complete for that node.
    fn parent_key_done_restoring(&mut self, parent_key: &str) {
        self.expansion_to_restore.remove(parent_key);
    }

    /// Advances expansion restoration one step: marks every present node in the
    /// restore set that still needs its children (`NotLoaded`) as loading, and
    /// fires a child fetch for each — in parallel. Re-invoked after each child
    /// batch arrives so deeper levels restore as they reappear. No-op when the
    /// restore set is empty.
    pub(crate) fn drive_expansion_restore(&mut self) {
        if !self.expansion_to_restore.is_empty() {
            // Take the set out to avoid borrowing `self` twice; the retain below
            // reinstates whatever still needs restoring.
            let restore = std::mem::take(&mut self.expansion_to_restore);
            let to_fetch = self.filtered_tree.nodes_needing_child_reload(&restore);
            self.request_children_batch(to_fetch);
            // Settle the set: drop ids that are present but no longer awaiting
            // work (already loaded/reopened). Keep ids still in flight and ids
            // not yet materialized — deeper levels appear once their parent's
            // children load.
            self.expansion_to_restore = restore;
            self.expansion_to_restore.retain(|id| {
                !self.filtered_tree.contains_item(id)
                    || self.pending_child_requests.contains_key(id.as_str())
            });
        }
    }

    pub(crate) fn apply_children_result(
        &mut self,
        request_id: u64,
        parent_key: String,
        result: JiraLoadResult,
    ) {
        self.command_log.extend(result.logs);
        if self.pending_child_requests.get(parent_key.as_str()) != Some(&request_id) {
            // The node was collapsed/reloaded, or a newer request superseded this.
            return;
        }
        self.pending_child_requests.remove(parent_key.as_str());

        // A seamless reload refreshes this subtree in place: swap the stale
        // children for the fresh set without collapsing or moving the view.
        if self.soft_reload_parents.contains(&parent_key) {
            // While the root query is still in flight, hold the result so the
            // node stays dimmed and the whole reload settles as one unit when
            // the roots land — rather than un-dimming this subtree early and
            // then re-dimming it, which reads as a flicker.
            if self.pending_reload_seamless {
                self.reload_children_buffer
                    .push((parent_key, result.issues));
                return;
            }
            self.soft_reload_parents.remove(&parent_key);
            self.settle_soft_reload(&parent_key, result.issues);
            return;
        }

        match result.issues {
            Ok(issues) => {
                self.filtered_tree
                    .add_children(&parent_key, tree_items_from_issues(issues));
                // Newly-arrived children may themselves be nodes whose expansion
                // is being restored; fetch and re-open them too.
                self.parent_key_done_restoring(&parent_key);
                self.drive_expansion_restore();
            }
            Err(error) => {
                self.filtered_tree.mark_children_failed(&parent_key);
                self.parent_key_done_restoring(&parent_key);
                self.notifications
                    .push(Notification::error("Could not load child issues", error.0));
            }
        }
    }

    /// Swaps a soft-reloaded subtree's stale children for the fresh set in
    /// place. On failure the stale subtree is kept and only the spinner clears.
    fn settle_soft_reload(
        &mut self,
        parent_key: &str,
        issues: Result<Vec<IssueSummary>, JiraError>,
    ) {
        match issues {
            Ok(issues) => {
                self.filtered_tree
                    .replace_children(parent_key, tree_items_from_issues(issues));
            }
            Err(error) => {
                self.filtered_tree.mark_children_loaded(parent_key);
                self.notifications.push(Notification::error(
                    "Could not refresh child issues",
                    error.0,
                ));
            }
        }
    }

    /// Applies a batched children load by fanning each parent's result through
    /// the same per-parent handling as a single load, so soft-reload, stale
    /// guards, and the expansion cascade behave identically. The shared query
    /// logs are recorded once for the whole batch.
    pub(crate) fn apply_children_batch_result(
        &mut self,
        results: Vec<(u64, String, Result<Vec<IssueSummary>, JiraError>)>,
        logs: Vec<CommandLogEntry>,
    ) {
        self.command_log.extend(logs);
        for (request_id, parent_key, issues) in results {
            self.apply_children_result(
                request_id,
                parent_key,
                JiraLoadResult {
                    issues,
                    next_page_token: None,
                    logs: Vec::new(),
                },
            );
        }
    }

    /// Records a filter change for debounced searching. Terms shorter than
    /// `SEARCH_MIN_CHARS` cancel any pending search and restore the browse view
    /// so the list never shows results that don't match the box. Longer terms
    /// (re)start the debounce window; the search fires from `tick` once typing
    /// pauses for `SEARCH_DEBOUNCE`.
    pub(crate) fn queue_search(&mut self, term: String) {
        if term.trim().chars().count() < SEARCH_MIN_CHARS {
            self.pending_search = None;
            self.restore_browse_view();
            return;
        }
        self.pending_search = Some((term, SEARCH_DEBOUNCE));
    }

    /// Counts down the pending search's debounce window and fires it once the
    /// window elapses. Called every `tick`; a no-op when no search is pending.
    pub(crate) fn advance_pending_search(&mut self, dt: std::time::Duration) {
        let Some((_, remaining)) = self.pending_search.as_mut() else {
            return;
        };
        *remaining = remaining.saturating_sub(dt);
        if remaining.is_zero()
            && let Some((term, _)) = self.pending_search.take()
        {
            self.run_search(term);
        }
    }

    /// Starts a server-side search for `term`, or restores the browse view when
    /// `term` is empty.
    pub(crate) fn run_search(&mut self, term: String) {
        if term.trim().is_empty() {
            self.restore_browse_view();
            return;
        }
        let Some(credentials) = self.credentials.clone() else {
            return;
        };
        // Entering search abandons any in-flight browse expansion restore and
        // lazy child loads; their results must not touch the flat search view.
        self.pending_child_requests.clear();
        self.expansion_to_restore.clear();
        // Likewise drop any in-flight seamless-reload paging/refresh so a late
        // root page or child batch can't merge into the search results.
        self.pending_roots_request_id = None;
        self.next_root_page_token = None;
        self.reload_root_ids = None;
        self.soft_reload_parents.clear();
        self.pending_reload_seamless = false;
        self.reload_children_buffer.clear();
        self.view = ListView::Search(term.clone());
        let request_id = self.next_request_id();
        self.search_request_id = Some(request_id);
        self.status = format!("Searching for \"{term}\"");
        self.pending_effects.push(AppEffect::SearchIssues {
            request_id,
            credentials,
            term,
            fields: self.current_fields_param(),
        });
    }

    pub(crate) fn apply_search_result(
        &mut self,
        request_id: u64,
        term: String,
        result: JiraLoadResult,
    ) {
        self.command_log.extend(result.logs);
        // Only the most recent search is applied (debounce-by-latest), and only
        // while still searching for the same term.
        if self.search_request_id != Some(request_id) || !self.view.is_searching_for(&term) {
            return;
        }
        self.search_request_id = None;

        match result.issues {
            Ok(issues) => {
                self.filtered_tree.set_flat(true);
                self.filtered_tree.set_items(
                    tree_items_from_issues(issues),
                    &std::collections::HashSet::new(),
                );
                // Highlights now follow the term that produced these results.
                self.applied_search_term = Some(term.clone());
                let count = self.filtered_tree.items().len();
                self.status = format!("{count} result(s) for \"{term}\".");
            }
            Err(error) => {
                self.status = error.0;
            }
        }
    }

    /// Reloads the browse tree from scratch after leaving search.
    pub(crate) fn restore_browse_view(&mut self) {
        if self.view == ListView::Browse {
            return;
        }
        self.view = ListView::Browse;
        self.search_request_id = None;
        self.applied_search_term = None;
        let Some(credentials) = self.credentials.clone() else {
            return;
        };
        self.status = String::from("Loading Jira issues");
        self.queue_jira_load(
            JiraLoadPurpose::Reload,
            credentials,
            crate::services::jira::ROOT_PAGE_SIZE,
        );
    }

    pub(crate) fn reload_list(&mut self) {
        if self.active_tab() != ApplicationTab::List {
            return;
        }

        let Some(credentials) = self.credentials.clone() else {
            self.status = String::from("No Jira credentials available for reload.");
            return;
        };

        // A seamless reload the existing tree in place rather than
        // tearing it down, so the selection, scroll position, and expanded
        // subtrees stay put — no anchoring to the root, no jump to the top.
        // Capture the currently-expanded nodes so their children are refreshed
        // in the background. Must be set after queue_jira_load, which clears the
        // restore set for non-reload loads.
        let expanded = self.filtered_tree.expanded_item_ids().clone();
        // Refetch the whole already-loaded extent in a single page so the reload
        // is one root query, not page-by-page.
        let extent = self
            .filtered_tree
            .root_count()
            .max(crate::services::jira::ROOT_PAGE_SIZE as usize)
            .min(crate::services::jira::CHILD_PAGE_SIZE as usize) as u32;
        self.status = String::from("Reloading Jira issues");
        self.queue_jira_load(JiraLoadPurpose::Reload, credentials, extent);
        // Grey out the expanded subtrees and fetch their fresh children right
        // away, in parallel with the root query, so the reload shows immediate
        // per-node feedback instead of waiting for the root response to return.
        // The nodes stay dimmed until the roots land: child results that arrive
        // first are buffered (see `apply_children_result`) and applied together
        // in `begin_seamless_reload`, so the reload reads as one operation.
        let to_fetch = self.filtered_tree.begin_soft_reload(&expanded);
        for parent in &to_fetch {
            self.soft_reload_parents.insert(parent.clone());
        }
        self.request_children_batch(to_fetch);
        self.pending_reload_seamless = true;
    }

    /// Refreshes the children of the selected tree node in place, keeping the
    /// stale subtree visible (greyed) until the fresh set arrives so the node
    /// never collapses or jumps. Does nothing when the selection has no loaded
    /// children (or there is no selection); only `Shift+R` reloads the whole
    /// list.
    pub(crate) fn reload_node(&mut self) {
        if self.active_tab() != ApplicationTab::List || self.view != ListView::Browse {
            return;
        }
        let Some(node_id) = self.filtered_tree.selected_item_id().map(str::to_owned) else {
            return;
        };
        // Refresh the node and any open descendant subtrees in place, in
        // parallel. `begin_soft_reload` marks each as loading without dropping
        // its children and returns the ids to refetch.
        let mut targets = self.filtered_tree.expanded_descendant_ids(&node_id);
        targets.insert(node_id.clone());
        let to_fetch = self.filtered_tree.begin_soft_reload(&targets);
        if to_fetch.is_empty() {
            return;
        }
        self.status = format!("Reloading {node_id}");
        for parent in &to_fetch {
            self.soft_reload_parents.insert(parent.clone());
        }
        self.request_children_batch(to_fetch);
    }
}
