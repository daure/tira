# Ratatui TUI Architecture Guidelines

Date: 2026-06-06

## Purpose

This document is the Ratatui architecture baseline for Tira, a keyboard-first Jira TUI. It combines reusable Ratatui practices with Tira-specific constraints for project layout, runtime lifecycle, state management, rendering, service boundaries, testing, and architecture decisions.

Use the generic guidance only where it does not conflict with `constitution.md` or the Jira-specific sections below.

## Tira project invariants

For Tira, this generic Ratatui architecture is constrained by `constitution.md`.

- Every keybinding resolves through configurable action bindings. Built-in defaults are allowed; hard-coded-only shortcuts are not.
- Every color resolves through semantic theme roles. Built-in themes are allowed; per-role overrides must be supported.
- The footer/status bar is reserved global chrome in normal mode and input mode. Screens may contribute context hints, but they may not remove it.
- Overflow shortcut help is an action, not a literal key special case. The built-in default binding is `?`; the UI must display the configured binding.
- Jira writes are explicit user-visible effects. Destructive or hard-to-reverse Jira writes require confirmation, and failed writes become visible UI state.
- Nerd Font icons may be used for scanability, but text carries meaning. Every icon has an ASCII/Unicode fallback or can be omitted when no sensible fallback exists.

## Design goals

A well-structured Ratatui app should be:

- **Recoverable:** terminal state is restored on normal exit, panic, and signal paths.
- **Responsive:** rendering and input stay usable while background work runs.
- **Testable:** state transitions, keymaps, services, and representative layouts can be tested without a real terminal.
- **Separable:** UI code does not own domain logic, external IO, protocol parsing, or durable persistence.
- **Navigable:** module boundaries make it obvious where lifecycle, state, effects, widgets, services, and domain models live.
- **Stable under refresh:** selections and focused items survive sorting, filtering, pagination, and snapshot refreshes.
- **Simple first:** start with one package and clear modules; extract crates only when boundaries prove useful.

## Core architecture principles

### 1. Centralize terminal lifecycle

Every Ratatui app needs one terminal lifecycle wrapper. Do not scatter raw-mode and alternate-screen logic through the application.

The wrapper owns:

- enabling/disabling raw mode;
- entering/leaving alternate screen;
- enabling/disabling mouse and paste modes if used;
- cursor visibility;
- terminal backend creation;
- panic hook cleanup;
- signal/shutdown cleanup where feasible;
- final restoration before returning to stdout/stderr output.

Recommended invariant: every path that enters terminal mode must restore the terminal before returning control to the shell.

### 2. Separate domain state from view state

Widgets should not own the business engine. Keep durable facts and UI navigation state separate.

- **Domain state:** parsed data, loaded records, protocol/domain models, workflow rules, cache snapshots, service results.
- **View state:** selected screen/tab, focused component, selected row key, scroll offsets, input buffers, modal state, table state, status messages.

Recommended invariant: `ui` modules do not call external APIs, parse protocol payloads, mutate durable persistence, or perform expensive domain computations. They render prepared state and emit actions.

### 3. Pick one update style deliberately

Use one primary event/update model instead of mixing many styles.

| Style | Fit | Tradeoff |
|---|---|---|
| Direct imperative mutation | Small local tools, simple controls, animation loops | Fast to write; harder to test and scale once async flows grow. |
| Action/message reducer | API-backed apps, editable multi-region apps, modals, background tasks | More upfront types; clearer effects, tests, and stale-result handling. |
| Component returns effects | Tabbed dashboards and live views where screens own local input | Keeps screen behavior local while preserving explicit cross-cutting effects. |

Recommended default for a new non-trivial Ratatui app:

```text
Terminal Event -> Input Handler -> Action -> update(AppState) -> Effects -> Service Results -> Action -> Render
```

This keeps IO outside widgets and makes behavior testable.

### 4. Use first-class mode, focus, screen, and modal state

One enum or small set of enums should answer:

- Which input mode is interpreting keys: normal, input, or visual?
- Which screen or tab is active?
- Which component has keyboard focus?
- Is a modal open?
- Is a text input active?

Avoid scattered booleans such as `show_help`, `editing`, `confirming_delete`, and `search_focused` when they can become one `InputMode`, `Screen`, `Focus`, or `ModalState` value.

### 5. Keep rendering compositional and boring

Prefer simple render functions and small widgets over a deep framework too early.

Recommended rendering structure:

- One top-level compositor owns global layout and overlay order.
- Screen/tab modules own their sub-layout.
- Shared widgets own repeated mechanics such as tables, inputs, popups, status bars, and help footers.
- Theme/chrome helpers centralize blocks, borders, colors, status styles, and focus styles.
- Render functions read prepared state and draw into the Ratatui `Frame` or `Buffer`; they do not perform IO, await futures, hold long locks, emit effects, or perform avoidable per-frame allocation.
- Precompute or cache visible rows, wrapped text, status/help strings, and icon labels when source data, config, or terminal width changes.
- Render only viewport-visible rows for large Jira lists and activity streams.

### 6. Background work communicates through events or snapshots

Long-running work should not block input or rendering.

Common patterns:

- **Async service events:** background tasks send loaded/failed/cancelled events back to the app.
- **Snapshot polling:** workers maintain cheap snapshots; the UI reads cheap handles or small immutable views on tick. Large collections use stable IDs plus cached visible rows/wrapped text recomputed only when source data, filters, or terminal width changes.
- **Thread event channel:** blocking libraries run in threads and send domain events over channels.
- **No background work:** local-only or replay-style apps can stay synchronous if rendering remains responsive.

Recommended default for API-backed apps: async service tasks return typed result actions. For continuously changing data, maintain snapshots and refresh on a timer.

### 7. Runtime errors should become user-visible state

Startup/config/auth/terminal errors may exit before entering the UI. Runtime domain failures should usually keep the terminal usable.

Examples:

- query failure becomes a panel error state;
- edit failure becomes a modal/status error;
- background worker failure becomes a degraded mode or warning;
- closed channels become shutdown signals, not panics;
- stale service responses are ignored by request ID or generation token.

### 8. Test seams with minimal load-bearing tests

High-value tests prove behavior across boundaries. Prefer one test that exercises a real path through modules over several tests that assert isolated implementation details.

Good first targets:

- config files merge into runtime settings;
- configured key input resolves to a named action and updates state;
- state renders important status/help content with a Ratatui test backend;
- Jira payloads map into domain models;
- Jira failures become visible UI errors;
- stale service responses cannot overwrite newer intent.

Use focused unit tests only for code with intrinsic branching risk: parsers, validation, state machines, conflict resolution, and stale response handling.

Avoid asserting every style cell, every default value, getters, pass-through wrappers, or framework behavior.

## Recommended package shape

Start with a modular single package. Extract workspace crates only when a boundary becomes independently reusable, platform-specific, security-sensitive, feature-gated, or large enough to need independent tests.

### Reference structure as features appear

Start with only the modules needed to make the first real vertical slice work. The tree below is a reference shape, not a scaffold checklist.

```text
src/
  main.rs                    # Thin binary: parse CLI, init logging, load config, call lib::run
  lib.rs                     # Public run() and module declarations

  cli.rs                     # CLI args and non-TUI command entry points
  config.rs                  # Config/keybinding/theme loading and merge
  error.rs                   # AppError, Result, terminal-safe error display helpers

  app/
    mod.rs                   # App construction and run loop
    state.rs                 # AppState: global view state and domain snapshots
    action.rs                # Action enum: terminal events, user intents, Jira results
    effect.rs                # Effect enum: Jira calls, persistence, clipboard/browser, quit
    update.rs                # update(&mut AppState, Action) -> Vec<Effect>
    keymap.rs                # KeyEvent -> Action mapping from configurable bindings
    focus.rs                 # InputMode, Focus, Screen, Modal identity

  tui/
    mod.rs                   # Tui wrapper: Terminal, enter/exit, draw, event stream
    event.rs                 # Terminal backend event -> Event conversion
    terminal.rs              # Raw mode, alternate screen, panic/signal cleanup

  ui/
    mod.rs                   # Top-level draw(frame, &AppState)
    layout.rs                # Shared layout helpers and responsive breakpoints
    theme.rs                 # Semantic theme tokens and style helpers
    chrome.rs                # Panel blocks, tabs, status bar, help footer
    screens/
      mod.rs
      issues.rs              # First issue list/detail screen
    widgets/
      mod.rs
      table.rs               # Reusable selectable/scrollable table
      input.rs               # Text input/search widgets
      popup.rs               # Modal frame helpers

  domain/
    mod.rs                   # Pure Jira-facing domain models and invariants
    ids.rs                   # IssueKey and other strong IDs/newtypes
    models.rs                # Issue, project, user, transition, comment models
    query.rs                 # JQL/project/filter/search models

  services/
    mod.rs                   # Jira service boundary and effect handles
    jira.rs                  # Jira API client/protocol mapping
    tasks.rs                 # async task spawning, cancellation, request IDs
```

Add these only when real pressure appears:

- `forms/*` for create/edit/comment flows with validation and dirty state.
- `ui/screens/settings.rs` for runtime settings once settings are editable in-app.
- `ui/screens/help.rs` when dynamic help outgrows a shared popup widget.
- `ui/widgets/markdown.rs` when Jira descriptions/comments need rich rendering.
- `services/cache.rs` when refresh behavior proves a cache is needed.
- `services/persistence.rs` when local history, saved filters, or favorites exist.
- `services/clipboard.rs` and `services/browser.rs` when those integrations ship.
- `testsupport/*` when integration tests need reusable fake services or render helpers.

### Extraction triggers

Extract crates only when there is a clear reason:

```text
crates/
  app-core/                  # Pure domain models, query/workflow, service traits
  app-client/                # External API/protocol mapping
  app-tui/                   # Ratatui UI shell
```

Useful extraction triggers:

- a domain engine needs reuse outside the TUI;
- platform-specific code needs feature gates or privilege boundaries;
- protocol/client code has independent release or test needs;
- compile times or dependency graphs become painful;
- security boundaries need stricter dependency control.

Do not start with crates for every folder. Module boundaries are enough until extraction pressure is real.

## Runtime lifecycle

1. `main` parses CLI and handles non-TUI commands early.
2. Load config from defaults, config file, environment variables, and CLI overrides.
3. Initialize logging and terminal-safe panic hooks.
4. Build service handles: client, cache, persistence, clipboard/browser adapters.
5. Build initial `AppState` from config and optional startup query/entity.
6. Enter terminal through `Tui::enter()`.
7. Start event stream and service task supervisor.
8. Run loop:
   - receive terminal event, tick, render event, or service result;
   - map raw input to `Action` through the keymap/input handler;
   - call `update` to mutate state and produce `Effect`s;
   - execute effects asynchronously or synchronously as appropriate;
   - render when state is dirty, on resize, or on scheduled render ticks.
9. On quit, cancel outstanding service tasks, persist volatile state if needed, restore the terminal, then print post-run messages or errors.

## Event, action, and effect model

Use three layers.

```rust
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Render,
    Service(ServiceEvent),
}

pub enum Action {
    QuitRequested,
    QuitConfirmed,
    FocusChanged(Focus),
    SearchSubmitted(JqlQuery),
    IssueSelected(IssueKey),
    IssueLoaded(IssueDetails),
    IssueLoadFailed { key: IssueKey, error: JiraError },
    IssueEditSubmitted(IssueEdit),
    OpenHelp,
    ModalOpened(Modal),
    ModalClosed,
    Tick,
    Render,
}

pub enum Effect {
    LoadIssue(IssueKey),
    RunSearch(JqlQuery),
    SubmitIssueEdit(IssueEdit),
    SaveConfig(ConfigPatch),
    CopyToClipboard(String),
    OpenInBrowser(String),
    Quit,
}
```

Rules:

- Raw terminal events become actions before reaching the reducer.
- Configured keybindings map from stable snake_case action IDs such as `open_help` to Rust actions such as `Action::OpenHelp`; key handlers do not special-case literal keys.
- `update` is the normal state mutation seam.
- `effects` execute IO and return actions or service events.
- Effects that can become stale carry request IDs or generation tokens.
- Long-running effects are cancellable or ignored when obsolete.
- Render code never executes effects.

## State model

Split state into durable categories.

```rust
pub struct AppState {
    pub running: bool,
    pub dirty: bool,
    pub mode: InputMode,
    pub focus: Focus,
    pub screen: Screen,
    pub modal: Option<ModalState>,
    pub config: RuntimeConfig,
    pub ui: UiState,
    pub data: DataState,
    pub tasks: TaskState,
    pub messages: MessageState,
}
```

Recommended state categories:

- `UiState`: selected tab, selected row key, scroll offsets, input buffers, table states, popup state.
- `InputMode`: normal, input, or visual. Visual mode represents multi-selection state and controls visual-mode key interpretation and footer behavior.
- `DataState`: current query/list results, detail cache, loading/error statuses, derived view models.
- `TaskState`: in-flight requests, request IDs, cancellation handles, generation tokens.
- `MessageState`: status bar text, clipboard confirmation, transient errors, background warnings.

Use stable domain IDs for selections, not row indexes. Recompute visible indexes from the selected ID after refresh, sort, filter, or pagination changes.

## Layout strategy

Use one top-level layout function. Keep the screen structure obvious.

```text
+------------------------------------------------------------+
| Header / tabs / active context                             |
+------------------------+-----------------------------------+
| Left: list/search/nav  | Right: details/editor/activity     |
|                        |                                   |
+------------------------+-----------------------------------+
| Status bar / hints, with active input coexisting as needed   |
+------------------------------------------------------------+
```

Responsive policy:

- **Wide terminal:** side-by-side panels.
- **Narrow terminal:** stacked or accordion panels.
- **Very narrow terminal:** one active panel plus status/help footer.

Rendering rules:

- `ui::draw` owns global layout, reserved chrome, and modal overlay order.
- The footer/status bar region is reserved before screen content is laid out in normal mode and input mode.
- Overlays may cover or dim content panels, but they must not obscure the footer/status bar while the footer invariant applies.
- Search/text input must coexist with required status and hints. It may truncate by documented priority or use a second footer/input row, but it must not replace the normal/input status bar.
- Each screen module owns only its sub-layout.
- Shared widgets own repeated mechanics: table selection/scroll, search input, popup frame, markdown/rich text, status bar.
- Styles come from theme helpers, not ad hoc colors in every screen.
- Layout functions should be deterministic enough for focused render tests with a Ratatui test backend; assert semantic regions, text, status, and help content, not every cell or style.

## Services and data boundaries

Keep Jira IO behind the effect executor. The UI emits actions; effects call the client; service results return as actions.

Prefer concrete clients and generic seams until a dynamic trait object is genuinely needed. Avoid normalizing boxed futures or `async_trait` into the default architecture just because it is convenient for examples. Use `async_trait` or boxed dynamic dispatch only when runtime substitution is required and the seam is kept off the render path.

Conceptual boundary:

```rust
pub struct JiraClient { /* auth, base URL, HTTP client */ }

impl JiraClient {
    pub async fn search_issues(&self, query: JqlQuery) -> Result<SearchPage>;
    pub async fn load_issue(&self, key: IssueKey) -> Result<IssueDetails>;
    pub async fn submit_issue_edit(&self, edit: IssueEdit) -> Result<IssueDetails>;
}
```

Recommended rules:

- Protocol structs live in `services/jira.rs` or `services/jira/*`.
- Domain models live in `domain/*`.
- UI-facing view models are derived in `domain/*` or `app/state.rs`.
- Widgets never parse protocol JSON and never call HTTP clients.
- Runtime cache lives in services or app data state, not widgets.
- Edits use typed requests plus validation before an effect is emitted.
- Pagination, auth, rate limits, and protocol expansion options stay behind the service boundary.

## Jira mapping

Map the generic architecture to Jira consistently.

| Architecture concept | Tira/Jira concept |
|---|---|
| Stable selection ID | `IssueKey`, not row index |
| Query state | JQL, project, assignee, status, text filters |
| Detail state | Issue fields, comments, transitions, changelog if loaded |
| Write effects | create issue, edit fields, transition, assign, comment |
| Confirmation state | destructive or hard-to-reverse Jira writes |
| Stale response guard | request ID or generation token per query/detail/edit flow |
| Visible result state | pending, success, failure, cancelled, stale ignored |

## Modals and forms

Use modal/form state rather than ad hoc booleans.

```rust
pub enum ModalState {
    Confirm(ConfirmState),
    Picker(PickerState),
    TextInput(TextInputState),
    Editor(EditorState),
    Form(FormState),
    Help(HelpState),
}
```

Modal rules:

- While a modal is open, it receives input first.
- Modal completion returns an `Action` or typed payload.
- Modal hints, validation errors, and submit status flow through modal state and the global footer/status bar. In normal and input mode, modal overlays must leave the footer visible or explicitly transition to a mode where the footer invariant does not apply.
- Use Jira field metadata when it materially improves validation or dynamic field handling. Otherwise model forms explicitly with typed state.
- Destructive or hard-to-reverse Jira writes require confirmation.
- External editor flows should show a diff/confirmation for destructive or hard-to-reverse writes when useful.
- Modal state should include validation errors, active field, dirty flag, and submit status.

## Configuration

Config should cover:

- Atlassian site, email, API token source, and default project in `~/.tira/config.toml`;
- theme selection and semantic color overrides in `~/.tira/tui.toml`;
- configurable keybindings by stable snake_case action ID in `~/.tira/keybindings.toml`;
- startup screen/query/project/issue key;
- list columns and visible fields;
- refresh interval/cache TTL when refresh exists;
- editor/browser/clipboard integration when those integrations exist;
- custom commands only if the app earns extensibility.
- icon mode, including Nerd Font preference and fallback behavior.

Merge order:

1. built-in defaults;
2. config files under `~/.tira/`: `config.toml`, `keybindings.toml`, and `tui.toml`;
3. environment variables;
4. CLI flags.

Keep config parsing separate from runtime state. Convert config into `RuntimeConfig` before entering the terminal. Secrets must never appear in logs, status bars, panic output, debug views, or test snapshots.

Keymap contract:

- External action IDs are stable snake_case strings such as `open_help`.
- Rust action variants may use idiomatic names such as `Action::OpenHelp`, but the config mapping must be explicit.
- Bindings may be global, mode-specific, screen-specific, panel-specific, or modal-specific.
- Precedence must be documented from most specific to least specific.
- Startup validation rejects unknown action IDs, ambiguous duplicate bindings in the same scope, and unbound required keyboard paths.
- Footer and help content are generated from the resolved keymap, never from hard-coded shortcut text.

Theme contract:

- Theme roles are centralized in a `ThemeRole` registry or equivalent typed structure.
- Every role has a built-in value in every built-in theme.
- Per-role overrides are validated at config load.
- Widgets construct styles through `Theme` or chrome helpers, not raw colors.
- At least one focused render test should prove that a color override changes the relevant semantic style without asserting broad cell-by-cell snapshots.

Icon contract:

- Icons are decoration or scan aids, not the only source of meaning.
- The app supports a Nerd Font icon set and a fallback set.
- Fallbacks use sensible ASCII or Unicode characters when they improve clarity.
- If no sensible fallback exists, omit the icon and keep the label/status text.
- Icon selection is centralized with theme/chrome helpers, not scattered through widgets.

## Error handling and shutdown

- Use typed errors at service/domain boundaries.
- Convert runtime service failures into `Action::...Failed` and render them.
- Exit only for unrecoverable startup/config/terminal errors.
- Treat closed channels as shutdown, not panic.
- Restore terminal in normal quit, panic hook, and signal path.
- Cancel or ignore stale async responses by request ID.
- Preserve the last useful UI state when showing recoverable errors.
- Print post-run errors only after the terminal is restored.

## Testing plan

Keep the suite small and load-bearing.

High-value integration tests:

1. Config files merge into `RuntimeConfig`, including keybindings and theme overrides.
2. A configured key input resolves to a named action, updates state, and renders the expected status/help content.
3. A Jira search/detail payload maps into domain state and preserves selection by `IssueKey`.
4. A Jira failure returns a typed failure action and renders a visible recoverable error.
5. A stale response is ignored when a newer request already changed intent.
6. Terminal lifecycle restores the shell on normal exit and the testable error path.

Targeted unit tests are acceptable for parsers, validation, state-machine edges, and stale-response guards. Avoid exhaustive reducer branch tests, broad whole-screen snapshots, and assertions over every terminal cell.

## Architecture decisions to follow

### ADR-1: Start modular single-package, not workspace-first

- **Decision:** Use one package with clear modules; extract crates only when boundaries prove reusable or security/platform-sensitive.
- **Why:** Single packages can remain understandable when boundaries are explicit. Workspaces pay off only when a domain engine, client, or platform layer has independent value.
- **Consequence:** Faster initial implementation. Requires discipline in module boundaries.

### ADR-2: Use Action/Effect update model for non-trivial apps

- **Decision:** Normalize events into `Action`, mutate state in `update`, and execute IO through `Effect` handlers.
- **Why:** Editable apps with async work, modals, and service calls need message clarity. Direct imperative mutation scales poorly once flows overlap.
- **Consequence:** More upfront types. Easier testing and fewer hidden side effects.

### ADR-3: UI owns view state; services own IO and cache

- **Decision:** Widgets and screens never call external services directly.
- **Why:** Keeping domain/protocol work outside render code preserves responsiveness and testability.
- **Consequence:** Requires view models and effect plumbing.

### ADR-4: Use stable identity for selection

- **Decision:** Store selected domain IDs/keys, not only selected row indexes.
- **Why:** Live-refresh UIs remain stable across sort/filter/snapshot changes when selection is key-based.
- **Consequence:** Tables need helper logic to map selected ID to visible row.

### ADR-5: Keep rendering shallow and compositional

- **Decision:** Prefer top-level layout functions, screen render modules, and small reusable widgets over a deep widget framework at the start.
- **Why:** Ratatui rendering is easiest to maintain when layout ownership is visible and widget abstractions stay small.
- **Consequence:** Some repetition is acceptable until reuse pressure is clear.

## Plan handoff

Recommended workstreams:

| Workstream | Purpose | Depends on | Validation |
|---|---|---|---|
| Config/keymap/theme contracts | config merge, named key actions, semantic theme roles | none | config -> runtime and key -> action integration test |
| Terminal foundation | `Tui`, terminal lifecycle, event stream, panic cleanup | none | enter/exit terminal, key/resize/tick events, cleanup on testable error path |
| App state/update | `AppState`, `Action`, `Effect`, `Focus`, `update` | config/keymap/theme contracts | behavior tests for representative state transitions |
| Layout/chrome | top-level layout, issue list/detail/help/status widgets, always-visible footer | terminal foundation + app state | configured key -> action -> state -> status/help render integration test |
| Jira boundary | domain models, Jira client, fixture-backed service adapter at the effect boundary | app state | fake search/detail/failure result flows into visible state |
| Async effects | request spawning, stale response handling, cancellation | Jira boundary | stale/failed/success response tests |
| Forms/modals | confirm/picker/text/form/editor state | app state + layout/chrome | modal input/update tests for real write flows |

Spike first if unknown:

- External API auth, pagination, and rate-limit behavior.
- Rich text or markup conversion.
- Attachment/image/editor/browser integrations.
- Terminal rendering performance for large lists/tables.
- Cross-platform terminal behavior for mouse, paste, alternate screen, and signals.

## Bottom-line recommendation

Build new Ratatui apps around this core loop:

```text
Terminal Event -> Keymap/Input Handler -> Action -> update(AppState) -> Effects -> Service Results -> Action -> Render
```

Start with a modular single package. Keep terminal lifecycle, app state, rendering, domain models, services, forms, and tests in separate modules. Extract crates only when the seams prove useful. Keep widgets render-focused, keep IO behind effects/services, and use stable domain identities for selection.
