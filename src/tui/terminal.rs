use std::{io, panic};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub struct Tui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl Tui {
    pub fn enter() -> io::Result<Self> {
        install_panic_hook();
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).inspect_err(|_| {
            let _ = disable_raw_mode();
        })?;
        if let Err(error) = execute!(stdout, EnableMouseCapture) {
            let _ = disable_raw_mode();
            let _ = execute!(stdout, LeaveAlternateScreen);
            return Err(error);
        }

        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(mut terminal) => {
                if let Err(error) = terminal.hide_cursor() {
                    let _ = disable_raw_mode();
                    let _ = execute!(
                        terminal.backend_mut(),
                        DisableMouseCapture,
                        LeaveAlternateScreen
                    );
                    let _ = terminal.show_cursor();
                    return Err(error);
                }
                Ok(Self { terminal })
            }
            Err(error) => {
                restore_terminal();
                Err(error)
            }
        }
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}

fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            restore_terminal();
            previous(info);
        }));
    });
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
}
