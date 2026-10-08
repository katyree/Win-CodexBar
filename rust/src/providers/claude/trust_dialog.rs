//! Answers Claude Code's workspace-trust dialog during the trust preflight.
//!
//! Older releases focus "Yes, I trust this folder"; newer ones (2.1.29x)
//! focus "No, exit". Pressing Enter blindly would quit on the new layout and
//! never save the trust, so the focused option is read from the screen.

use super::cli_screen;

const PRESS_ENTER: &[&str] = &["\r"];
const SELECT_NEXT_THEN_ENTER: &[&str] = &["\x1b[B", "\r"];

/// Claude has fully drawn the dialog (confirm hint included) and one of the
/// two options shows the focus marker; returns the keys that accept trust.
pub(super) fn trust_dialog_keys(raw_screen: &str) -> Option<&'static [&'static str]> {
    let screen = cli_screen::render(raw_screen, true).to_lowercase();
    if !screen.contains("quick safety check") || !screen.contains("enter to confirm") {
        return None;
    }
    let trust_line = screen
        .lines()
        .find(|line| line.contains("yes, i trust this folder"))?;
    let exit_line = screen.lines().find(|line| line.contains("no, exit"))?;
    if has_focus_marker(trust_line) {
        Some(PRESS_ENTER)
    } else if has_focus_marker(exit_line) {
        Some(SELECT_NEXT_THEN_ENTER)
    } else {
        None
    }
}

fn has_focus_marker(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with('❯') || line.starts_with('>')
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOCUS_ON_NO: &str = "\x1b[2J\x1b[H Accessing workspace:\r\n\r\n C:/probe\r\n\r\n Quick safety check: Is this a project you created or one you trust?\r\n\r\n\x1b[36m \u{276f} No, exit\x1b[39m\r\n   Yes, I trust this folder\r\n\r\n Enter to confirm \u{b7} Esc to cancel\r\n";
    const FOCUS_ON_YES: &str = "Accessing workspace:\r\n\r\nC:/probe\r\n\r\nQuick safety check: Is this a project you created or one you trust?\r\n\r\n\u{276f} 1. Yes, I trust this folder\r\n  2. No, exit\r\n\r\nEnter to confirm \u{b7} Esc to cancel\r\n";
    // 2.1.293 skips blanks with absolute column jumps (CSI n G).
    const COLUMN_JUMPS: &str = "Quick\x1b[7Gsafety\x1b[14Gcheck: you trust this\r\n\x1b[1G\u{276f}\x1b[3GNo,\x1b[7Gexit\r\n\x1b[3GYes,\x1b[8GI\x1b[10Gtrust\x1b[16Gthis\x1b[21Gfolder\r\nEnter to confirm";

    #[test]
    fn focus_on_no_moves_down_before_enter() {
        assert_eq!(trust_dialog_keys(FOCUS_ON_NO), Some(SELECT_NEXT_THEN_ENTER));
    }

    #[test]
    fn focus_on_yes_just_presses_enter() {
        assert_eq!(trust_dialog_keys(FOCUS_ON_YES), Some(PRESS_ENTER));
    }

    #[test]
    fn column_jumps_do_not_glue_words_together() {
        assert_eq!(
            trust_dialog_keys(COLUMN_JUMPS),
            Some(SELECT_NEXT_THEN_ENTER)
        );
    }

    #[test]
    fn waits_until_the_dialog_is_fully_drawn() {
        let partial = FOCUS_ON_NO.split("Enter to confirm").next().unwrap();
        assert_eq!(trust_dialog_keys(partial), None);
        assert_eq!(trust_dialog_keys("Welcome to Claude Code"), None);
    }
}
