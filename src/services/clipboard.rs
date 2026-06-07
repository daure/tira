use std::{
    io::Write,
    process::{Command, Stdio},
};

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    for command in [
        ClipboardCommand::new("wl-copy", &[]),
        ClipboardCommand::new("xclip", &["-selection", "clipboard"]),
        ClipboardCommand::new("xsel", &["--clipboard", "--input"]),
    ] {
        if command.copy(text).is_ok() {
            return Ok(());
        }
    }

    Err(String::from("no supported clipboard command found"))
}

struct ClipboardCommand<'a> {
    program: &'a str,
    args: &'a [&'a str],
}

impl<'a> ClipboardCommand<'a> {
    const fn new(program: &'a str, args: &'a [&'a str]) -> Self {
        Self { program, args }
    }

    fn copy(&self, text: &str) -> Result<(), String> {
        let mut child = Command::new(self.program)
            .args(self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;

        let Some(mut stdin) = child.stdin.take() else {
            return Err(String::from("clipboard command did not open stdin"));
        };

        stdin
            .write_all(text.as_bytes())
            .map_err(|error| error.to_string())?;
        drop(stdin);

        child
            .wait()
            .map_err(|error| error.to_string())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("clipboard command exited with {status}"))
                }
            })
    }
}
