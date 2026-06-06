use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tira::{App, KeyBindings, config, draw};

fn main() -> io::Result<()> {
    let app = config::load_jira_credentials().map_or_else(App::default, App::from_credentials);
    let keybindings = KeyBindings::load();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &keybindings, app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    keybindings: &KeyBindings,
    app: App,
) -> io::Result<()> {
    let mut app = app;
    let mut last_tick = Instant::now();

    while app.is_running() {
        let dt = last_tick.elapsed();
        last_tick = Instant::now();
        app.tick(dt);

        terminal.draw(|frame| draw(frame, &app))?;

        let timeout = if app.is_animating() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(250)
        };

        #[allow(clippy::collapsible_if)]
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key, keybindings);
                last_tick = Instant::now();
            }
        }
    }

    Ok(())
}
