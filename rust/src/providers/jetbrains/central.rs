use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::process::Command;

use crate::core::{ProviderError, RateWindow, UsageSnapshot};

pub fn find_central_cli() -> Option<PathBuf> {
    which::which("central").ok().or_else(|| {
        let executable = if cfg!(windows) {
            "central.exe"
        } else {
            "central"
        };
        dirs::home_dir()
            .map(|home| home.join(".local/bin").join(executable))
            .filter(|path| path.is_file())
    })
}

pub(super) async fn fetch(
    cli: &Path,
    timeout_seconds: u64,
) -> Result<UsageSnapshot, ProviderError> {
    let mut command = Command::new(cli);
    command
        .args(["limit", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_seconds.clamp(1, 15)),
        command.output(),
    )
    .await
    .map_err(|_| ProviderError::Other("JetBrains Central usage request timed out".into()))?
    .map_err(|_| ProviderError::Other("Unable to run JetBrains Central CLI".into()))?;
    if !output.status.success() {
        // Never replace a failed live/account request with another IDE's cached quota.
        return Err(ProviderError::Other(
            "JetBrains Central could not read usage. Run central limit in a terminal to check your connection and sign-in.".into(),
        ));
    }
    parse(&output.stdout)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CentralQuota {
    #[serde(deserialize_with = "super::quota::number_or_string")]
    used_dollars: f64,
    #[serde(deserialize_with = "super::quota::number_or_string")]
    max_dollars: f64,
    refill_next: Option<i64>,
}

fn parse(json: &[u8]) -> Result<UsageSnapshot, ProviderError> {
    let quota: CentralQuota = serde_json::from_slice(json).map_err(|_| {
        ProviderError::Parse("JetBrains Central returned an unrecognized usage response".into())
    })?;
    if !quota.used_dollars.is_finite()
        || quota.used_dollars < 0.0
        || !quota.max_dollars.is_finite()
        || quota.max_dollars <= 0.0
    {
        return Err(ProviderError::Parse(
            "JetBrains Central returned an invalid quota".into(),
        ));
    }
    let resets_at = quota
        .refill_next
        .and_then(DateTime::<Utc>::from_timestamp_millis);
    let primary = RateWindow::with_details(
        quota.used_dollars / quota.max_dollars * 100.0,
        None,
        resets_at,
        Some(format!(
            "{:.2} / {:.2} credits used",
            quota.used_dollars, quota.max_dollars
        )),
    )
    .with_description_as_detail();
    Ok(UsageSnapshot::new(primary).with_login_method("JetBrains Central"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_live_credit_totals_and_reset_without_exposing_account_details() {
        let quota = parse(br#"{"email":"test@example.invalid","licenseName":"Test Organization","usedDollars":"4.28","maxDollars":"40.00","refillNext":1793491199999}"#).unwrap();
        assert!((quota.primary.used_percent - 10.7).abs() < 0.0001);
        assert_eq!(
            quota.primary.reset_description.as_deref(),
            Some("4.28 / 40.00 credits used")
        );
        assert_eq!(
            quota.primary.resets_at.unwrap().timestamp_millis(),
            1793491199999
        );
        assert!(quota.account_email.is_none());
    }

    #[test]
    fn missing_or_zero_limit_is_not_reported_as_zero_usage() {
        assert!(parse(br#"{}"#).is_err());
        assert!(parse(br#"{"usedDollars":0,"maxDollars":0}"#).is_err());
    }
}
