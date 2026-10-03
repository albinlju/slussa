//! The terminal while the TUI runs.

use ratatui::DefaultTerminal;

/// The terminal in TUI mode. Dropping it hands the terminal back as it was,
/// also when the event loop returns early or a panic unwinds past it.
pub(in crate::tui) struct TerminalGuard {
    pub(in crate::tui) terminal: DefaultTerminal,
}

impl TerminalGuard {
    pub(in crate::tui) fn enter() -> std::io::Result<Self> {
        // `try_init` turns raw mode on before the steps that can still fail, and
        // there is no guard yet to undo that, so a failure restores here.
        let terminal = ratatui::try_init().inspect_err(|_| ratatui::restore())?;
        let guard = Self { terminal };
        // If this fails the guard is dropped, which restores the terminal.
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste);
        ratatui::restore();
    }
}
