use std::io;
use std::process::Command;
use std::sync::atomic::Ordering;

use color_eyre::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::DefaultTerminal;
use ratatui::widgets::Clear;

use super::App;

impl App {
    fn resume_tui(&self, terminal: &mut DefaultTerminal) -> Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Clear, area);
        })?;
        Ok(())
    }

    pub(super) fn edit_unit(
        &mut self,
        terminal: &mut DefaultTerminal,
        unit_name: &str,
    ) -> Result<()> {
        self.event_listener_enabled.store(false, Ordering::Relaxed);

        if let Err(error) = disable_raw_mode() {
            self.error_message = Some(format!("Failed to disable raw mode: {error}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, LeaveAlternateScreen) {
            self.resume_tui(terminal)?;
            self.error_message = Some(format!("Failed to leave alternate screen: {error}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        if let Err(error) = terminal.show_cursor() {
            self.resume_tui(terminal)?;
            self.error_message = Some(format!("Failed to show cursor: {error}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let mut command = Command::new("systemctl");
        command.arg("edit").arg("--full");
        if self.selected_tab_index == 1 {
            command.arg("--user");
        }
        let edit_error = match command.arg(unit_name).status() {
            Ok(status) if status.success() => None,
            Ok(_) => Some("'systemctl edit' failed. Try running the program with sudo!".into()),
            Err(error) => Some(format!("Error executing systemctl: {error}")),
        };

        if let Err(error) = self.resume_tui(terminal) {
            self.error_message = Some(format!("Failed to return to TUI: {error}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        self.error_message = edit_error;
        self.event_listener_enabled.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub(super) fn suspend_tui(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        unsafe {
            libc::raise(libc::SIGTSTP);
        }

        self.resume_tui(terminal)?;
        Ok(())
    }
}
