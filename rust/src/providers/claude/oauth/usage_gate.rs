//! Gate in front of the Claude OAuth usage endpoint (nesszer/Win-CodexBar#775).
//!
//! * After a 429 no request goes out until `max(Retry-After, 5 min * 2^(n-1))`
//!   (capped at 30 min) has passed. `n` counts consecutive 429s and resets only
//!   on a success, never because the block expired. The block ignores the
//!   token, so a token refresh does not lift it.
//! * The block lives in a small JSON file under `%LOCALAPPDATA%\CodexBar\`, so
//!   every CodexBar process of the user shares it. An in-memory copy keeps it
//!   working when the file cannot be read or written.
//! * A successful response is served again for 3 minutes to the same sign-in
//!   (keyed by a SHA-256 token fingerprint). The response is kept in memory
//!   only: the file holds no token, fingerprint or response data.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;

const BASE_BACKOFF_MS: i64 = 5 * 60 * 1000;
const MAX_BACKOFF_MS: i64 = 30 * 60 * 1000;
const MIN_POLL_INTERVAL_MS: i64 = 3 * 60 * 1000;
const FILE_NAME: &str = "claude-oauth-usage-gate.json";

/// What the caller should do before hitting the endpoint.
#[derive(Debug, PartialEq)]
pub(super) enum Decision<T> {
    Blocked(Duration),
    Cached(T),
    Proceed,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
struct Persisted {
    blocked_until_ms: i64,
    consecutive: u32,
}

pub(super) struct UsageGate<T> {
    path: Option<PathBuf>,
    state: Persisted,
    last_success: Option<(String, i64, T)>,
}

pub(super) fn token_fingerprint(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) fn default_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("CodexBar").join(FILE_NAME))
}

pub(super) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn backoff_ms(retry_after: Duration, consecutive: u32) -> i64 {
    let exponential = BASE_BACKOFF_MS.saturating_mul(1i64 << consecutive.saturating_sub(1).min(10));
    let retry_after = i64::try_from(retry_after.as_millis()).unwrap_or(i64::MAX);
    retry_after.max(exponential).min(MAX_BACKOFF_MS)
}

impl<T: Clone> UsageGate<T> {
    pub(super) fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            state: Persisted::default(),
            last_success: None,
        }
    }

    /// Merge the shared file into memory. A later block, or a cleared state
    /// written by another process after a success, replaces ours.
    fn refresh(&mut self) {
        let Some(path) = &self.path else { return };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        if let Ok(disk) = serde_json::from_str::<Persisted>(&text) {
            self.state = disk;
        }
    }

    fn persist(&self) {
        let Some(path) = &self.path else { return };
        let Ok(json) = serde_json::to_vec(&self.state) else {
            return;
        };
        if let Err(err) = crate::atomic_file::write_atomic(path, &json) {
            tracing::debug!("Claude OAuth usage gate not persisted: {err}");
        }
    }

    pub(super) fn check(&mut self, now_ms: i64, fingerprint: &str) -> Decision<T> {
        self.refresh();
        if self.state.blocked_until_ms > now_ms {
            let remaining = (self.state.blocked_until_ms - now_ms) as u64;
            return Decision::Blocked(Duration::from_millis(remaining));
        }
        if let Some((fp, at, response)) = &self.last_success
            && fp == fingerprint
            && now_ms >= *at
            && now_ms - *at < MIN_POLL_INTERVAL_MS
        {
            return Decision::Cached(response.clone());
        }
        Decision::Proceed
    }

    /// Record a 429 and return how long requests are blocked.
    pub(super) fn record_rate_limit(&mut self, now_ms: i64, retry_after: Duration) -> Duration {
        self.refresh();
        self.state.consecutive = self.state.consecutive.saturating_add(1);
        let backoff = backoff_ms(retry_after, self.state.consecutive);
        self.state.blocked_until_ms = now_ms.saturating_add(backoff);
        self.persist();
        Duration::from_millis(backoff as u64)
    }

    pub(super) fn record_success(&mut self, now_ms: i64, fingerprint: &str, response: T) {
        self.refresh();
        self.last_success = Some((fingerprint.to_string(), now_ms, response));
        if self.state != Persisted::default() {
            self.state = Persisted::default();
            self.persist();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60 * 1000;
    const NOW: i64 = 1_000_000_000_000;

    fn gate(dir: &tempfile::TempDir) -> UsageGate<String> {
        UsageGate::new(Some(dir.path().join(FILE_NAME)))
    }

    fn blocked_mins(d: Decision<String>) -> u64 {
        match d {
            Decision::Blocked(d) => d.as_secs().div_ceil(60),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn consecutive_429s_escalate_even_after_the_block_expired() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = gate(&dir);
        let mut now = NOW;
        for expected in [5, 10, 20, 30, 30] {
            let wait = g.record_rate_limit(now, Duration::from_secs(1));
            assert_eq!(wait.as_secs(), expected * 60);
            now += i64::try_from(expected).unwrap() * MIN + 1;
            assert_eq!(g.check(now, "fp"), Decision::Proceed);
        }
    }

    #[test]
    fn retry_after_wins_when_longer_and_is_capped() {
        let mut g = gate(&tempfile::tempdir().unwrap());
        assert_eq!(
            g.record_rate_limit(NOW, Duration::from_secs(12 * 60))
                .as_secs(),
            720
        );
        let mut g = gate(&tempfile::tempdir().unwrap());
        assert_eq!(
            g.record_rate_limit(NOW, Duration::from_secs(3 * 3600))
                .as_secs(),
            1800
        );
    }

    #[test]
    fn success_resets_the_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = gate(&dir);
        g.record_rate_limit(NOW, Duration::ZERO);
        g.record_rate_limit(NOW + 6 * MIN, Duration::ZERO);
        g.record_success(NOW + 20 * MIN, "fp", "ok".into());
        assert_eq!(
            g.record_rate_limit(NOW + 30 * MIN, Duration::ZERO)
                .as_secs(),
            300
        );
    }

    #[test]
    fn block_is_shared_through_the_file_and_ignores_the_token() {
        let dir = tempfile::tempdir().unwrap();
        gate(&dir).record_rate_limit(NOW, Duration::ZERO);
        let mut other = gate(&dir);
        assert_eq!(blocked_mins(other.check(NOW + MIN, "another-token")), 4);
        assert_eq!(
            other.check(NOW + 5 * MIN + 1, "another-token"),
            Decision::Proceed
        );
        // The count survives in the file, so the next 429 doubles.
        assert_eq!(
            other
                .record_rate_limit(NOW + 6 * MIN, Duration::ZERO)
                .as_secs(),
            600
        );
    }

    #[test]
    fn memory_copy_works_without_a_usable_file() {
        let dir = tempfile::tempdir().unwrap();
        // The path's parent is a regular file, so reads and writes fail.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let mut g: UsageGate<String> = UsageGate::new(Some(blocker.join(FILE_NAME)));
        g.record_rate_limit(NOW, Duration::ZERO);
        assert_eq!(blocked_mins(g.check(NOW + MIN, "fp")), 4);
        let mut none: UsageGate<String> = UsageGate::new(None);
        none.record_rate_limit(NOW, Duration::ZERO);
        assert_eq!(blocked_mins(none.check(NOW, "fp")), 5);
    }

    #[test]
    fn corrupt_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), b"{not json").unwrap();
        let mut g = gate(&dir);
        assert_eq!(g.check(NOW, "fp"), Decision::Proceed);
        g.record_rate_limit(NOW, Duration::ZERO);
        assert_eq!(blocked_mins(g.check(NOW, "fp")), 5);
    }

    #[test]
    fn successful_polls_are_throttled_per_sign_in() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = gate(&dir);
        g.record_success(NOW, "fp-a", "usage-a".into());
        assert_eq!(
            g.check(NOW + 2 * MIN, "fp-a"),
            Decision::Cached("usage-a".into())
        );
        assert_eq!(g.check(NOW + 2 * MIN, "fp-b"), Decision::Proceed);
        assert_eq!(g.check(NOW + 3 * MIN, "fp-a"), Decision::Proceed);
    }

    #[test]
    fn file_holds_no_token_fingerprint_or_response() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = gate(&dir);
        let fp = token_fingerprint("secret-token");
        g.record_rate_limit(NOW, Duration::ZERO);
        g.record_success(NOW, &fp, "response-body".into());
        g.record_rate_limit(NOW, Duration::ZERO);
        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(
            !text.contains("secret-token") && !text.contains(&fp) && !text.contains("response")
        );
        assert_eq!(fp.len(), 64);
    }
}
