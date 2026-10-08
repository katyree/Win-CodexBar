//! Provider-owned chart snapshots and quota-window projections.
//!
//! The desktop shell should only map these provider-neutral values to its
//! transport DTOs. In particular, Claude's daily history and quota history
//! must come from one project-tree walk so that parsing, deduplication, and
//! completeness decisions cannot diverge between chart fields.

use crate::codex_costs::codex_quota_windows_from_cache;
use crate::core::{JsonlScanner, ProviderId, RateWindow};
use crate::cost_reporting_period::cost_bucket_zone;
use crate::cost_scanner::{CostScanner, CostSummary};
use crate::providers::claude::quota_history::{
    ClaudeQuotaHistoryOptions, ClaudeQuotaResetObservation, aggregate_claude_quota_windows,
};
use crate::providers::claude::reset_observations;
use crate::providers::codex::reset_observations as codex_reset_observations;
use chrono::{DateTime, NaiveDate, Utc};
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Default)]
pub struct ProviderChartSnapshot {
    pub daily_cost: Vec<(String, Option<f64>)>,
    pub daily_tokens: Vec<(String, u64)>,
    /// Claude incomplete proxy-request count per local day (upstream 0.60.5 #3688).
    pub daily_incomplete: Vec<(String, u32)>,
    pub tokens_incomplete: bool,
    /// Claude cost for the current bucket-zone calendar day; `None` when the
    /// day has no usage or its cost is not fully priced.
    pub today_cost: Option<f64>,
    /// Claude tokens (input + output + cache) for the current bucket-zone day.
    pub today_tokens: Option<u64>,
    pub local_summary: Option<CostSummary>,
    pub quota_window_history: Option<QuotaWindowHistorySnapshot>,
}

#[derive(Debug, Clone)]
pub struct QuotaWindowHistorySnapshot {
    pub provider_id: String,
    pub account_scope: Option<String>,
    pub windows: Vec<QuotaWindowSnapshot>,
    pub history_coverage_established: bool,
}

#[derive(Debug, Clone)]
pub struct QuotaWindowSnapshot {
    pub offset: usize,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub total_tokens: Option<u64>,
    pub total_cost_usd: Option<f64>,
    pub tokens_are_complete: bool,
    pub cost_is_complete: bool,
    pub entry_count: usize,
    pub boundaries_are_estimated: bool,
}

/// Build provider-owned chart inputs for the providers with special history
/// behavior. Other providers continue through the shared legacy chart path in
/// the desktop shell.
pub fn build_chart_snapshot(
    provider_id: &str,
    account_scope: Option<&str>,
    live_window: Option<&RateWindow>,
    cancel: Option<&AtomicBool>,
) -> Option<ProviderChartSnapshot> {
    match provider_id {
        "claude" => Some(build_claude_chart_snapshot(
            account_scope,
            live_window,
            cancel,
        )),
        "codex" => Some(ProviderChartSnapshot {
            quota_window_history: build_codex_quota_history(account_scope, live_window),
            ..ProviderChartSnapshot::default()
        }),
        _ => None,
    }
}

fn build_claude_chart_snapshot(
    account_scope: Option<&str>,
    live_window: Option<&RateWindow>,
    cancel: Option<&AtomicBool>,
) -> ProviderChartSnapshot {
    let account_scope = account_scope
        .map(str::trim)
        .filter(|scope| !scope.is_empty());

    let scan = CostScanner::new(30).scan_claude_chart_snapshot_with_cancel(cancel);
    let now = Utc::now();
    let quota_window_history = account_scope.and_then(|account_scope| {
        let live_window = live_window?;
        let observations =
            load_and_persist_reset_observations(account_scope, Some(live_window), now);
        let report = aggregate_claude_quota_windows(
            account_scope,
            &scan.quota_history.records,
            ClaudeQuotaHistoryOptions {
                live_reset_at: live_window.resets_at,
                window_minutes: live_window.window_minutes,
                observations: &observations,
                now,
                max_windows: 4,
                history_coverage_established: scan.quota_history.history_coverage_established,
            },
        );
        (!report.windows.is_empty()).then(|| QuotaWindowHistorySnapshot {
            provider_id: "claude".to_string(),
            account_scope: Some(report.account_scope),
            windows: report
                .windows
                .into_iter()
                .enumerate()
                .map(|(offset, window)| QuotaWindowSnapshot {
                    offset,
                    start: window.start,
                    end: window.end,
                    total_tokens: window.total_tokens,
                    total_cost_usd: window.total_cost_usd,
                    tokens_are_complete: window.tokens_are_complete,
                    cost_is_complete: window.cost_is_complete,
                    entry_count: usize::try_from(window.entry_count).unwrap_or(usize::MAX),
                    boundaries_are_estimated: window.boundaries_are_estimated,
                })
                .collect(),
            history_coverage_established: report.history_coverage_established,
        })
    });

    let today = cost_bucket_zone().date(now);
    let (today_cost, today_tokens) =
        claude_today_usage(&scan.daily_cost, &scan.daily_tokens, today);
    ProviderChartSnapshot {
        today_cost,
        today_tokens,
        daily_cost: scan.daily_cost,
        daily_tokens: scan.daily_tokens,
        daily_incomplete: scan.daily_incomplete,
        tokens_incomplete: !scan.summary.history_coverage_established,
        local_summary: Some(scan.summary),
        quota_window_history,
    }
}

/// Today's Claude cost and tokens from the per-day chart buckets, which use the
/// same bucket zone as `today`. Zero or unpriced days yield `None`.
fn claude_today_usage(
    daily_cost: &[(String, Option<f64>)],
    daily_tokens: &[(String, u64)],
    today: NaiveDate,
) -> (Option<f64>, Option<u64>) {
    let key = today.format("%Y-%m-%d").to_string();
    let cost = daily_cost
        .iter()
        .find(|(day, _)| *day == key)
        .and_then(|(_, cost)| *cost)
        .filter(|cost| *cost > 0.0);
    let tokens = daily_tokens
        .iter()
        .find(|(day, _)| *day == key)
        .map(|(_, tokens)| *tokens)
        .filter(|tokens| *tokens > 0);
    (cost, tokens)
}

fn load_and_persist_reset_observations(
    account_scope: &str,
    live_window: Option<&RateWindow>,
    now: DateTime<Utc>,
) -> Vec<ClaudeQuotaResetObservation> {
    let Some(config_root) = dirs::config_dir().map(|root| root.join("CodexBar")) else {
        return Vec::new();
    };
    let Some(reset_at) = live_window.and_then(|window| window.resets_at) else {
        return reset_observations::load_reset_observations(&config_root, account_scope)
            .unwrap_or_default();
    };

    let incoming = [ClaudeQuotaResetObservation {
        account_scope: account_scope.to_string(),
        captured_at: now,
        resets_at: reset_at,
    }];
    match reset_observations::merge_and_persist_reset_observations(
        &config_root,
        account_scope,
        &incoming,
    ) {
        Ok(result) => result.observations,
        Err(_) => reset_observations::load_reset_observations(&config_root, account_scope)
            .unwrap_or_default(),
    }
}

/// Load the persisted Codex reset observations and record the live window's
/// reset on this refresh (upstream 0.62.0 #3358). Read failures never break
/// the chart: an error degrades to the previously stored observations.
fn load_and_persist_codex_reset_observations(
    live_window: &RateWindow,
    now: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    let Some(reset_at) = live_window.resets_at else {
        return Vec::new();
    };
    let config_root = match dirs::config_dir().map(|root| root.join("CodexBar")) {
        Some(root) => root,
        None => return Vec::new(),
    };
    let scope = codex_reset_observations::CODEX_ACCOUNT_SCOPE;
    match codex_reset_observations::merge_and_persist_reset_observation(
        &config_root,
        scope,
        reset_at,
        now,
    ) {
        Ok(result) => result
            .observations
            .iter()
            .map(|observation| observation.resets_at)
            .collect(),
        Err(_) => codex_reset_observations::load_reset_observations(&config_root, scope)
            .unwrap_or_default()
            .iter()
            .map(|observation| observation.resets_at)
            .collect(),
    }
}

fn build_codex_quota_history(
    account_scope: Option<&str>,
    live_window: Option<&RateWindow>,
) -> Option<QuotaWindowHistorySnapshot> {
    let live_window = live_window?;
    let cache = JsonlScanner::load_cache(ProviderId::Codex, None);
    let now = Utc::now();
    let observed_next_resets = load_and_persist_codex_reset_observations(live_window, now);
    let windows =
        codex_quota_windows_from_cache(&cache, Some(live_window), &observed_next_resets, now, 4);
    if windows.is_empty() {
        return None;
    }

    let history_coverage_established = !cache.codex_scan_incomplete
        && cache.codex_pending_paths.is_empty()
        && cache.scan_since_key.is_some()
        && cache.scan_until_key.is_some();
    Some(QuotaWindowHistorySnapshot {
        provider_id: "codex".to_string(),
        account_scope: account_scope
            .map(str::trim)
            .filter(|scope| !scope.is_empty())
            .map(ToOwned::to_owned),
        windows: windows
            .into_iter()
            .map(|window| QuotaWindowSnapshot {
                offset: window.offset,
                start: window.start,
                end: window.end,
                total_tokens: window.total_tokens,
                total_cost_usd: window.total_cost_usd,
                tokens_are_complete: window.tokens_are_complete,
                cost_is_complete: window.cost_is_complete,
                entry_count: window.entry_count,
                boundaries_are_estimated: window.boundaries_are_estimated,
            })
            .collect(),
        history_coverage_established,
    })
}

#[cfg(test)]
mod claude_today_tests {
    use super::claude_today_usage;
    use crate::cost_reporting_period::CostTimeZone;
    use chrono::{TimeZone, Utc};

    type Fixture = (Vec<(String, Option<f64>)>, Vec<(String, u64)>);

    fn fixture() -> Fixture {
        (
            vec![
                ("2026-10-06".to_string(), Some(1.5)),
                ("2026-10-07".to_string(), Some(2.25)),
                ("2026-10-08".to_string(), None),
            ],
            vec![
                ("2026-10-06".to_string(), 100),
                ("2026-10-07".to_string(), 750),
                ("2026-10-08".to_string(), 0),
            ],
        )
    }

    #[test]
    fn today_is_the_zone_calendar_day_not_utc() {
        let (cost, tokens) = fixture();
        // 2026-10-07 20:00 UTC is already 2026-10-08 05:00 in Tokyo.
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 20, 0, 0).unwrap();
        let tokyo = CostTimeZone::Named(chrono_tz::Asia::Tokyo).date(now);
        assert_eq!(claude_today_usage(&cost, &tokens, tokyo), (None, None));
        let utc = CostTimeZone::UTC.date(now);
        assert_eq!(
            claude_today_usage(&cost, &tokens, utc),
            (Some(2.25), Some(750))
        );
    }

    #[test]
    fn today_ignores_other_days_and_missing_buckets() {
        let (cost, tokens) = fixture();
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        assert_eq!(
            claude_today_usage(&cost, &tokens, day),
            (Some(1.5), Some(100))
        );
        let missing = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(claude_today_usage(&cost, &tokens, missing), (None, None));
    }
}
