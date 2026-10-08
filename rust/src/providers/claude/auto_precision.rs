//! Keeps Claude Auto's two usage sources on the same percentage precision.
//!
//! The OAuth endpoint reports fractional utilization (for example 7.6), while
//! Claude Code's `/usage` screen prints `Math.floor(utilization)` (7). Auto
//! alternates between the two when OAuth is rate limited, and the UI rounds to
//! the nearest whole number, so the same utilization showed as 8 from OAuth
//! and 7 from the CLI (issue #776). Flooring the OAuth values in Auto gives
//! both sources the CLI's precision, so they agree on the same utilization.

use crate::core::{ProviderFetchResult, RateWindow};

/// Floor every quota window in an OAuth result to a whole percent, as the CLI does.
pub(super) fn floor_to_cli_precision(result: &mut ProviderFetchResult) {
    let usage = &mut result.usage;
    floor_window(&mut usage.primary);
    for window in [
        usage.secondary.as_mut(),
        usage.model_specific.as_mut(),
        usage.tertiary.as_mut(),
    ]
    .into_iter()
    .flatten()
    {
        floor_window(window);
    }
    for named in &mut usage.extra_rate_windows {
        floor_window(&mut named.window);
    }
}

fn floor_window(window: &mut RateWindow) {
    if window.used_percent.is_finite() {
        window.used_percent = window.used_percent.floor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::UsageSnapshot;

    fn result(session: f64, weekly: f64) -> ProviderFetchResult {
        let mut usage = UsageSnapshot::new(RateWindow::new(session));
        usage.secondary = Some(RateWindow::new(weekly));
        ProviderFetchResult::new(usage, "oauth")
    }

    fn shown(result: &ProviderFetchResult) -> (f64, f64) {
        // The UI and tray display `round(used_percent)`.
        let weekly = result.usage.secondary.as_ref().expect("weekly");
        (
            result.usage.primary.used_percent.round(),
            weekly.used_percent.round(),
        )
    }

    #[test]
    fn oauth_and_cli_agree_after_flooring() {
        // The same underlying utilization, read from OAuth (fractional) and
        // from the CLI (already floored by Claude Code).
        let cli = result(7.0, 9.0);
        let mut oauth = result(7.6, 9.5);
        assert_eq!(shown(&oauth), (8.0, 10.0), "unfixed OAuth flips to 8/10");

        floor_to_cli_precision(&mut oauth);
        let polls = [&oauth, &cli, &oauth, &cli];
        assert!(polls.iter().all(|poll| shown(poll) == (7.0, 9.0)));
    }

    #[test]
    fn real_increases_still_show() {
        let mut later = result(8.2, 10.1);
        floor_to_cli_precision(&mut later);
        assert_eq!(shown(&later), (8.0, 10.0));
    }

    #[test]
    fn whole_and_exhausted_values_are_unchanged() {
        let mut full = result(100.0, 0.0);
        floor_to_cli_precision(&mut full);
        assert_eq!(shown(&full), (100.0, 0.0));
    }
}
