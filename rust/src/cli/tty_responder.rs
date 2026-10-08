//! Keys the TTY runner sends in reaction to what the child has printed.

use super::tty_runner::TtyCommandOptions;
use std::io::Write;
use std::time::Duration;

/// Inspects output not yet answered and returns the key presses to send, if
/// any. Lets a caller react to what the screen shows (for example which
/// dialog option has focus) instead of to a fixed substring.
pub type ScreenResponder = fn(&str) -> Option<&'static [&'static str]>;

/// Upper bound on responder answers per run, so a screen that never changes
/// cannot make the runner type forever.
const MAX_SCREEN_RESPONSES: usize = 6;
/// Pause between the key presses of one responder answer.
const SCREEN_RESPONSE_KEY_GAP: Duration = Duration::from_millis(150);

/// Send every not-yet-fired `send_on_substrings` entry whose trigger is in `buffer`.
pub(super) fn fire_substring_triggers(
    options: &TtyCommandOptions,
    buffer: &str,
    triggered: &mut std::collections::HashSet<String>,
    writer: &mut impl Write,
) {
    for (trigger, keys) in &options.send_on_substrings {
        if !triggered.contains(trigger) && buffer.contains(trigger) {
            let normalized = keys.replace('\n', "\r\n");
            // Best-effort send-trigger input; a closed PTY drops the write.
            let _trigger_written = write!(writer, "{}", normalized);
            // Best-effort flush after a send-trigger write.
            let _trigger_flushed = writer.flush();
            triggered.insert(trigger.clone());
        }
    }
}

/// Tracks how much output the screen responder has already answered.
#[derive(Default)]
pub(super) struct ResponderState {
    answered_up_to: usize,
    answers: usize,
}

impl ResponderState {
    /// Offer the output since the last answer to the responder and send its keys.
    pub(super) fn answer(
        &mut self,
        options: &TtyCommandOptions,
        buffer: &str,
        writer: &mut impl Write,
    ) {
        let Some(responder) = options.screen_responder else {
            return;
        };
        if self.answers >= MAX_SCREEN_RESPONSES || self.answered_up_to >= buffer.len() {
            return;
        }
        let Some(keys) = responder(&buffer[self.answered_up_to..]) else {
            return;
        };
        for (index, key) in keys.iter().enumerate() {
            if index > 0 {
                std::thread::sleep(SCREEN_RESPONSE_KEY_GAP);
            }
            // Best-effort key press; a closed PTY drops the write.
            let _key_written = writer.write_all(key.as_bytes());
            // Best-effort flush after a responder key press.
            let _key_flushed = writer.flush();
        }
        self.answered_up_to = buffer.len();
        self.answers += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_responder_answers_once_per_new_output() {
        fn on_ready(screen: &str) -> Option<&'static [&'static str]> {
            screen.contains("READY").then_some(&["a", "b"][..])
        }
        let options = TtyCommandOptions::new().with_screen_responder(on_ready);
        let mut state = ResponderState::default();
        let mut sent = Vec::new();
        state.answer(&options, "booting", &mut sent);
        assert!(sent.is_empty());
        state.answer(&options, "booting READY", &mut sent);
        assert_eq!(sent, b"ab");
        // The answered READY is not offered again; only new output counts.
        state.answer(&options, "booting READY", &mut sent);
        state.answer(&options, "booting READY more", &mut sent);
        assert_eq!(sent, b"ab");
        state.answer(&options, "booting READY more READY", &mut sent);
        assert_eq!(sent, b"abab");
    }

    #[test]
    fn substring_triggers_fire_once_on_already_buffered_output() {
        let options = TtyCommandOptions::new().with_send_on_substring("Enter", "go\n");
        let mut triggered = std::collections::HashSet::new();
        let mut sent = Vec::new();
        fire_substring_triggers(&options, "Enter to confirm", &mut triggered, &mut sent);
        fire_substring_triggers(&options, "Enter to confirm", &mut triggered, &mut sent);
        assert_eq!(sent, b"go\r\n");
    }
}
