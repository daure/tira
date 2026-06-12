mod support;

use ratatui::{Terminal, backend::TestBackend};
use tira::{
    App, AppEffect, AppEvent, KeyBindings,
    config::JiraCredentials,
    domain::date::{civil_from_days, today_days},
    draw,
    services::jira::{
        SprintState, TimelineData, TimelineEpic, TimelineEpicStats, TimelineIssue, TimelineSprint,
    },
};

use support::{ctrl, key, project, rendered_text, shift};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

#[test]
fn timeline_loads_at_startup_with_credentials() {
    let mut app = App::from_credentials(JiraCredentials {
        site: String::from("https://example.atlassian.net"),
        email: String::from("test@example.com"),
        api_key: String::from("test"),
        default_project: String::from("DPP"),
    });

    let effects = app.take_effects();

    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::LoadJiraProject { .. })),
        "startup queues the issue/project load"
    );
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::LoadTimeline { .. })),
        "startup queues the timeline load"
    );
}

/// Builds an app on the Timeline tab with a load queued but not yet delivered,
/// returning the request id the load was issued under.
fn timeline_app_pending() -> (App, u64) {
    let mut app = App::with_issues_projects_and_users(
        Vec::new(),
        vec![project("DPP", "DICE")],
        Vec::new(),
        "DPP",
    );
    let bindings = KeyBindings::default();

    // Leader + t switches to the Timeline tab, which lazily queues the load.
    app.handle_key(ctrl('x'), &bindings);
    app.handle_key(key('t'), &bindings);

    let request_id = app
        .take_effects()
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadTimeline { request_id, .. } => Some(request_id),
            _ => None,
        })
        .expect("timeline load was queued on tab switch");
    (app, request_id)
}

/// Builds an app on the Timeline tab with one epic loaded via the real
/// load/event path, plus the request id the load was issued under.
fn timeline_app() -> (App, Terminal<TestBackend>) {
    let (mut app, request_id) = timeline_app_pending();

    let today = today_days();
    let data = TimelineData {
        epics: vec![TimelineEpic {
            key: String::from("DPP-1"),
            summary: String::from("Checkout revamp"),
            status: String::from("In Progress"),
            done: false,
            start_day: None,
            end_day: None,
            sprint_ids: vec![196],
            stats: TimelineEpicStats {
                to_do: 1,
                in_progress: 1,
                done: 2,
            },
            children: vec![TimelineIssue {
                key: String::from("DPP-2"),
                summary: String::from("Shopping cart"),
                status: String::from("In Progress"),
                issue_type: String::from("Story"),
                done: false,
                start_day: None,
                end_day: None,
                sprint_ids: vec![196],
            }],
        }],
        sprints: vec![TimelineSprint {
            id: 196,
            name: String::from("DICE Sprint 196"),
            state: SprintState::Active,
            start_day: Some(today - 7),
            end_day: Some(today + 7),
        }],
    };
    app.handle_event(AppEvent::TimelineLoaded {
        request_id,
        timeline: Ok(data),
        logs: Vec::new(),
    });

    let terminal = Terminal::new(TestBackend::new(160, 20)).expect("test terminal");
    (app, terminal)
}

#[test]
fn timeline_renders_epic_row_sprint_pill_today_marker_and_current_month() {
    let (app, mut terminal) = timeline_app();
    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");
    let (screen, _) = rendered_text(&terminal);

    assert!(screen.contains("DPP-1"), "epic key shown: {screen:?}");
    assert!(screen.contains("Checkout revamp"), "epic summary shown");
    // Numbered sprint renders with the short S<n> label, not the full name.
    assert!(screen.contains("S196"), "sprint short label shown");
    assert!(screen.contains('\u{e0b0}'), "right sprint cap drawn");
    assert!(!screen.contains('▼'), "today triangle hidden");
    // The proportional month header renders neighbouring months around today.
    // (The current month's own label can sit under the today triangle, so
    // assert on the adjacent months, which never are.)
    let (_, month, _) = civil_from_days(today_days());
    let prev = MONTHS[((month + 10) % 12) as usize];
    let next = MONTHS[(month % 12) as usize];
    assert!(
        screen.contains(prev) || screen.contains(next),
        "neighbouring month ({prev} or {next}) in header"
    );
    // The active sprint straddles today, so the epic draws a bounded timeline range.
    assert!(screen.contains('●'), "epic range caps drawn");
    assert!(screen.contains('━'), "epic range line drawn");
    // Children are hidden until the epic is expanded.
    assert!(!screen.contains("DPP-2"), "child hidden while collapsed");
}

#[test]
fn expanding_epic_reveals_child_rows() {
    let (mut app, mut terminal) = timeline_app();
    let bindings = KeyBindings::default();

    // `l` expands the selected epic (tree semantics), revealing its children.
    app.handle_key(key('l'), &bindings);

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw timeline");
    let (screen, _) = rendered_text(&terminal);

    assert!(
        screen.contains("DPP-2"),
        "child shown after expand: {screen:?}"
    );
    assert!(screen.contains("Shopping cart"), "child summary shown");
}

#[test]
fn slash_filters_timeline_rows_locally_with_fuzzy_search() {
    let (mut app, mut terminal) = timeline_app();
    let bindings = KeyBindings::default();

    app.handle_key(key('/'), &bindings);
    app.handle_key(key('c'), &bindings);
    app.handle_key(key('a'), &bindings);
    app.handle_key(key('r'), &bindings);
    app.handle_key(key('t'), &bindings);

    assert!(app.is_timeline_filter_focused());
    assert_eq!(app.timeline_filter(), "cart");

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw filtered timeline");
    let (screen, _) = rendered_text(&terminal);

    assert!(!screen.contains("Timeline — epics scheduled by sprint"));
    assert!(screen.contains("DPP-1"), "matching child's epic retained");
    assert!(screen.contains("DPP-2"), "matching child shown: {screen:?}");
    assert!(
        screen.contains("Shopping cart"),
        "matching child summary shown"
    );

    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(0, 1)].symbol(),
        "\u{f002}",
        "filter starts at left edge"
    );

    let highlight_bg = app.theme().highlight_bg();
    let highlighted_match_chars = buffer
        .content()
        .iter()
        .filter(|cell| cell.bg == highlight_bg && matches!(cell.symbol(), "c" | "a" | "r" | "t"))
        .count();
    assert!(
        highlighted_match_chars >= 4,
        "filtered timeline text should highlight matched characters"
    );
}

#[test]
fn stale_timeline_response_is_dropped() {
    let (mut app, request_id) = timeline_app_pending();

    // A response under a request id that is not the pending one (e.g. a
    // superseded load) must be ignored entirely.
    let today = today_days();
    let stale = TimelineData {
        epics: vec![TimelineEpic {
            key: String::from("STALE-9"),
            summary: String::from("Should never render"),
            status: String::from("To Do"),
            done: false,
            start_day: None,
            end_day: None,
            sprint_ids: Vec::new(),
            stats: TimelineEpicStats::default(),
            children: Vec::new(),
        }],
        sprints: vec![TimelineSprint {
            id: 1,
            name: String::from("DICE Sprint 1"),
            state: SprintState::Active,
            start_day: Some(today - 7),
            end_day: Some(today + 7),
        }],
    };
    app.handle_event(AppEvent::TimelineLoaded {
        request_id: request_id + 1000,
        timeline: Ok(stale),
        logs: Vec::new(),
    });

    // The stale payload was discarded: nothing loaded, still awaiting the real
    // response.
    assert!(app.timeline().data().is_none(), "stale data not stored");
    assert!(app.timeline().is_loading(), "still awaiting pending load");

    let mut terminal = Terminal::new(TestBackend::new(160, 20)).expect("test terminal");
    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");
    let (screen, _) = rendered_text(&terminal);
    assert!(
        !screen.contains("STALE-9"),
        "stale epic not rendered: {screen:?}"
    );
}

#[test]
fn timeline_selected_row_renders_with_green_selection_foreground_and_background() {
    let (app, mut terminal) = timeline_app();
    let theme = app.theme();
    let selected_bg = theme.selected_bg();
    let selected_fg = theme.selected_fg();

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");

    let buffer = terminal.backend().buffer();
    let area = *buffer.area();
    let mut selected_cell_found = false;
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            if x < 40 && cell.symbol() == "C" && buffer[(x + 1, y)].symbol() == "h" {
                if cell.fg == selected_fg && cell.bg == selected_bg {
                    selected_cell_found = true;
                }
            }
        }
    }
    assert!(
        selected_cell_found,
        "selected timeline epic row should render with selected fg and bg colors"
    );
}

#[test]
fn selected_timeline_range_and_overlapping_header_are_green() {
    let (app, mut terminal) = timeline_app();
    let success_fg = app.theme().success_fg();
    let selected_sprint_bg = darker(success_fg);

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");

    let buffer = terminal.backend().buffer();
    assert!(
        buffer_cells(buffer).any(|(_, _, cell)| {
            ["●", "━"].contains(&cell.symbol()) && cell.fg == success_fg
        }),
        "selected timeline range should render green"
    );
    assert!(
        text_has_bg(buffer, "S196", selected_sprint_bg),
        "selected range should highlight overlapping sprint label darker green"
    );
    let (_, month, _) = civil_from_days(today_days());
    assert!(
        text_has_bg(buffer, MONTHS[(month - 1) as usize], success_fg),
        "selected range should highlight overlapping month label background green"
    );
}

fn darker(color: ratatui::style::Color) -> ratatui::style::Color {
    match color {
        ratatui::style::Color::Rgb(red, green, blue) => {
            ratatui::style::Color::Rgb(red / 2, green / 2, blue / 2)
        }
        other => other,
    }
}

#[test]
fn selected_timeline_percent_is_bucket_colored_without_selection_background() {
    let (app, mut terminal) = timeline_app();
    let theme = app.theme();

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");

    let buffer = terminal.backend().buffer();
    let (x, y) = find_text(buffer, "50%").expect("selected epic percentage rendered");
    let cell = &buffer[(x, y)];
    assert_eq!(cell.fg, theme.accent_fg(), "50% uses the <75% bucket");
    assert_ne!(
        cell.bg,
        theme.selected_bg(),
        "percent complete should not use selected-row highlight"
    );
}

#[test]
fn timeline_percent_complete_uses_four_color_buckets() {
    let (mut app, request_id) = timeline_app_pending();
    app.handle_event(AppEvent::TimelineLoaded {
        request_id,
        timeline: Ok(TimelineData {
            epics: vec![
                timeline_epic_with_stats(
                    "DPP-1",
                    TimelineEpicStats {
                        to_do: 4,
                        in_progress: 0,
                        done: 0,
                    },
                ),
                timeline_epic_with_stats(
                    "DPP-2",
                    TimelineEpicStats {
                        to_do: 3,
                        in_progress: 0,
                        done: 1,
                    },
                ),
                timeline_epic_with_stats(
                    "DPP-3",
                    TimelineEpicStats {
                        to_do: 2,
                        in_progress: 0,
                        done: 2,
                    },
                ),
                timeline_epic_with_stats(
                    "DPP-4",
                    TimelineEpicStats {
                        to_do: 0,
                        in_progress: 0,
                        done: 4,
                    },
                ),
            ],
            sprints: Vec::new(),
        }),
        logs: Vec::new(),
    });
    let theme = app.theme();
    let mut terminal = Terminal::new(TestBackend::new(160, 20)).expect("test terminal");

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");

    let buffer = terminal.backend().buffer();
    assert_text_fg(buffer, "0%", theme.muted_fg());
    assert_text_fg(buffer, "25%", theme.warning_fg());
    assert_text_fg(buffer, "50%", theme.accent_fg());
    assert_text_fg(buffer, "100%", theme.success_fg());
}

#[test]
fn timeline_shift_z_expands_all_epics() {
    let (mut app, mut terminal) = timeline_app();
    let bindings = KeyBindings::default();

    app.handle_key(shift('z'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw timeline");

    let (screen, _) = rendered_text(&terminal);
    assert!(
        screen.contains("DPP-2"),
        "child shown after expand all: {screen:?}"
    );
}

#[test]
fn selected_child_timeline_range_uses_selected_color() {
    let (mut app, mut terminal) = timeline_app();
    let bindings = KeyBindings::default();
    let selected_range_fg = app.theme().success_fg();

    app.handle_key(shift('z'), &bindings);
    app.handle_key(key('j'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw timeline");

    let buffer = terminal.backend().buffer();
    assert!(
        buffer_cells(buffer).any(|(_, _, cell)| {
            ["●", "━"].contains(&cell.symbol()) && cell.fg == selected_range_fg
        }),
        "selected child timeline range should use selected green"
    );
}

#[test]
fn epic_and_child_bars_use_their_own_date_positions() {
    let (mut app, request_id) = timeline_app_pending();
    let today = today_days();
    app.handle_event(AppEvent::TimelineLoaded {
        request_id,
        timeline: Ok(TimelineData {
            epics: vec![TimelineEpic {
                key: String::from("DPP-20"),
                summary: String::from("Parent starts later"),
                status: String::from("In Progress"),
                done: false,
                start_day: Some(today - 14),
                end_day: Some(today + 14),
                sprint_ids: vec![2],
                stats: TimelineEpicStats {
                    to_do: 1,
                    in_progress: 0,
                    done: 0,
                },
                children: vec![
                    TimelineIssue {
                        key: String::from("DPP-27"),
                        summary: String::from("Child starts earlier"),
                        status: String::from("In Progress"),
                        issue_type: String::from("Story"),
                        done: false,
                        start_day: Some(today - 21),
                        end_day: Some(today - 7),
                        sprint_ids: vec![1],
                    },
                    TimelineIssue {
                        key: String::from("DPP-29"),
                        summary: String::from("Child ends later"),
                        status: String::from("To Do"),
                        issue_type: String::from("Story"),
                        done: false,
                        start_day: Some(today + 7),
                        end_day: Some(today + 21),
                        sprint_ids: vec![2],
                    },
                ],
            }],
            sprints: vec![
                TimelineSprint {
                    id: 1,
                    name: String::from("DICE Sprint 1"),
                    state: SprintState::Closed,
                    start_day: Some(today - 21),
                    end_day: Some(today - 7),
                },
                TimelineSprint {
                    id: 2,
                    name: String::from("DICE Sprint 2"),
                    state: SprintState::Active,
                    start_day: Some(today + 7),
                    end_day: Some(today + 21),
                },
            ],
        }),
        logs: Vec::new(),
    });
    let bindings = KeyBindings::default();
    app.handle_key(key('l'), &bindings);
    let mut terminal = Terminal::new(TestBackend::new(160, 18)).expect("test terminal");

    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw timeline");

    let buffer = terminal.backend().buffer();
    let parent_x = first_timeline_range_x(buffer, "DPP-20").expect("parent range starts");
    let child_x = first_timeline_range_x(buffer, "DPP-27").expect("child range starts");
    let parent_end_x = last_timeline_range_x(buffer, "DPP-20").expect("parent range ends");
    let last_child_end_x = last_timeline_range_x(buffer, "DPP-29").expect("last child range ends");
    let (sprint_2_x, _) = find_text(buffer, "S2").expect("later sprint header rendered");
    assert!(
        child_x < parent_x,
        "child assigned to the earlier sprint should start before parent: child={child_x}, parent={parent_x}"
    );
    assert!(
        parent_end_x < last_child_end_x,
        "last child should end after parent: parent={parent_end_x}, child={last_child_end_x}"
    );
    assert!(
        parent_x < sprint_2_x,
        "parent start should sit before the later sprint label center: parent={parent_x}, S2={sprint_2_x}"
    );
}

#[test]
fn epic_bar_falls_back_to_child_sprints_when_epic_has_no_schedule() {
    let (mut app, request_id) = timeline_app_pending();
    let today = today_days();
    app.handle_event(AppEvent::TimelineLoaded {
        request_id,
        timeline: Ok(TimelineData {
            epics: vec![TimelineEpic {
                key: String::from("DPP-30"),
                summary: String::from("Parent without own schedule"),
                status: String::from("In Progress"),
                done: false,
                start_day: None,
                end_day: None,
                sprint_ids: Vec::new(),
                stats: TimelineEpicStats {
                    to_do: 1,
                    in_progress: 0,
                    done: 0,
                },
                children: vec![TimelineIssue {
                    key: String::from("DPP-31"),
                    summary: String::from("Scheduled child"),
                    status: String::from("In Progress"),
                    issue_type: String::from("Story"),
                    done: false,
                    start_day: None,
                    end_day: None,
                    sprint_ids: vec![1],
                }],
            }],
            sprints: vec![TimelineSprint {
                id: 1,
                name: String::from("DICE Sprint 1"),
                state: SprintState::Active,
                start_day: Some(today - 7),
                end_day: Some(today + 7),
            }],
        }),
        logs: Vec::new(),
    });
    let mut terminal = Terminal::new(TestBackend::new(160, 18)).expect("test terminal");

    terminal
        .draw(|frame| draw(frame, &app, &KeyBindings::default()))
        .expect("draw timeline");

    assert!(
        first_timeline_range_x(terminal.backend().buffer(), "DPP-30").is_some(),
        "epic without own schedule should still render from child sprint span"
    );
}

#[test]
fn timeline_navigation_pans_right_to_fully_show_selected_range() {
    let (mut app, mut terminal) = timeline_app_with_distant_ranges();
    let bindings = KeyBindings::default();
    let selected_range_fg = app.theme().success_fg();

    app.handle_key(key('j'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw timeline");

    assert_selected_range_visible(terminal.backend().buffer(), "DPP-2", selected_range_fg);
}

#[test]
fn timeline_navigation_pans_left_to_fully_show_selected_range() {
    let (mut app, mut terminal) = timeline_app_with_distant_ranges();
    let bindings = KeyBindings::default();
    let selected_range_fg = app.theme().success_fg();

    app.handle_key(key('j'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw future timeline");
    app.handle_key(key('k'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw past timeline");

    assert_selected_range_visible(terminal.backend().buffer(), "DPP-1", selected_range_fg);
}

#[test]
fn timeline_manual_horizontal_scroll_is_not_blocked_by_selected_range() {
    let (mut app, mut terminal) = timeline_app_with_distant_ranges();
    let bindings = KeyBindings::default();
    let selected_range_fg = app.theme().success_fg();

    app.handle_key(key('j'), &bindings);
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw autopanned timeline");
    assert_selected_range_visible(terminal.backend().buffer(), "DPP-2", selected_range_fg);

    for _ in 0..6 {
        app.handle_key(shift('h'), &bindings);
    }
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("set manual scroll target");
    app.tick(std::time::Duration::from_millis(250));
    terminal
        .draw(|frame| draw(frame, &app, &bindings))
        .expect("draw manually scrolled timeline");

    let visible_range_cells =
        selected_range_visible_cells(terminal.backend().buffer(), "DPP-2", selected_range_fg);
    assert!(
        visible_range_cells < 20,
        "manual horizontal scroll should be allowed to move away from selected range, saw {visible_range_cells} cells"
    );
}

fn timeline_app_with_distant_ranges() -> (App, Terminal<TestBackend>) {
    let (mut app, request_id) = timeline_app_pending();
    let today = today_days();
    app.handle_event(AppEvent::TimelineLoaded {
        request_id,
        timeline: Ok(TimelineData {
            epics: vec![
                timeline_epic_with_child("DPP-1", "DPP-11", 1),
                timeline_epic_with_child("DPP-2", "DPP-22", 2),
            ],
            sprints: vec![
                TimelineSprint {
                    id: 1,
                    name: String::from("DICE Sprint 1"),
                    state: SprintState::Closed,
                    start_day: Some(today - 165),
                    end_day: Some(today - 145),
                },
                TimelineSprint {
                    id: 2,
                    name: String::from("DICE Sprint 2"),
                    state: SprintState::Future,
                    start_day: Some(today + 300),
                    end_day: Some(today + 320),
                },
            ],
        }),
        logs: Vec::new(),
    });
    let terminal = Terminal::new(TestBackend::new(120, 18)).expect("test terminal");
    (app, terminal)
}

fn timeline_epic_with_child(key: &str, child_key: &str, sprint_id: i64) -> TimelineEpic {
    TimelineEpic {
        key: key.to_owned(),
        summary: format!("{key} summary"),
        status: String::from("In Progress"),
        done: false,
        start_day: None,
        end_day: None,
        sprint_ids: vec![sprint_id],
        stats: TimelineEpicStats {
            to_do: 1,
            in_progress: 0,
            done: 0,
        },
        children: vec![TimelineIssue {
            key: child_key.to_owned(),
            summary: format!("{child_key} summary"),
            status: String::from("In Progress"),
            issue_type: String::from("Story"),
            done: false,
            start_day: None,
            end_day: None,
            sprint_ids: vec![sprint_id],
        }],
    }
}

fn assert_selected_range_visible(
    buffer: &ratatui::buffer::Buffer,
    key: &str,
    fg: ratatui::style::Color,
) {
    let visible_range_cells = selected_range_visible_cells(buffer, key, fg);
    assert!(
        visible_range_cells >= 20,
        "selected row should show full 20-cell timeline range, saw {visible_range_cells}"
    );
}

fn first_timeline_range_x(buffer: &ratatui::buffer::Buffer, key: &str) -> Option<u16> {
    let (_, y) = find_text(buffer, key)?;
    (0..buffer.area().width).find(|x| ["●", "━"].contains(&buffer[(*x, y)].symbol()))
}

fn last_timeline_range_x(buffer: &ratatui::buffer::Buffer, key: &str) -> Option<u16> {
    let (_, y) = find_text(buffer, key)?;
    (0..buffer.area().width)
        .rev()
        .find(|x| ["●", "━"].contains(&buffer[(*x, y)].symbol()))
}

fn selected_range_visible_cells(
    buffer: &ratatui::buffer::Buffer,
    key: &str,
    fg: ratatui::style::Color,
) -> usize {
    let (_, y) = find_text(buffer, key).expect("selected issue key rendered");
    (0..buffer.area().width)
        .filter(|x| {
            let cell = &buffer[(*x, y)];
            ["●", "━"].contains(&cell.symbol()) && cell.fg == fg
        })
        .count()
}

fn timeline_epic_with_stats(key: &str, stats: TimelineEpicStats) -> TimelineEpic {
    TimelineEpic {
        key: key.to_owned(),
        summary: format!("{key} summary"),
        status: String::from("In Progress"),
        done: stats.percent_done() >= 100,
        start_day: None,
        end_day: None,
        sprint_ids: Vec::new(),
        stats,
        children: Vec::new(),
    }
}

fn assert_text_fg(buffer: &ratatui::buffer::Buffer, needle: &str, fg: ratatui::style::Color) {
    assert!(
        text_has_fg(buffer, needle, fg),
        "{needle} should render with {fg:?}"
    );
}

fn buffer_cells(
    buffer: &ratatui::buffer::Buffer,
) -> impl Iterator<Item = (u16, u16, &ratatui::buffer::Cell)> {
    let area = *buffer.area();
    (0..area.height).flat_map(move |y| (0..area.width).map(move |x| (x, y, &buffer[(x, y)])))
}

fn text_has_fg(buffer: &ratatui::buffer::Buffer, needle: &str, fg: ratatui::style::Color) -> bool {
    find_text(buffer, needle).is_some_and(|(start_x, y)| {
        needle
            .chars()
            .enumerate()
            .all(|(offset, _)| buffer[(start_x + offset as u16, y)].fg == fg)
    })
}

fn text_has_bg(buffer: &ratatui::buffer::Buffer, needle: &str, bg: ratatui::style::Color) -> bool {
    find_text(buffer, needle).is_some_and(|(start_x, y)| {
        needle
            .chars()
            .enumerate()
            .all(|(offset, _)| buffer[(start_x + offset as u16, y)].bg == bg)
    })
}

fn find_text(buffer: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    let area = *buffer.area();
    let needle_chars = needle.chars().collect::<Vec<_>>();
    if needle_chars.is_empty() || needle_chars.len() > area.width as usize {
        return None;
    }
    for y in 0..area.height {
        for x in 0..=area.width - needle_chars.len() as u16 {
            if needle_chars
                .iter()
                .enumerate()
                .all(|(offset, ch)| buffer[(x + offset as u16, y)].symbol() == ch.to_string())
            {
                return Some((x, y));
            }
        }
    }
    None
}
