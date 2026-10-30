//! Reusable TUI primitives, widgets, and panic safety hooks for BashMenu.

use std::panic;

/// Registers a global panic hook ensuring terminal raw mode and alternate screen
/// are cleanly restored before printing panic diagnostics.
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_hook_registration() {
        // Verifies the hook can be registered without panicking
        install_panic_hook();
    }
}
