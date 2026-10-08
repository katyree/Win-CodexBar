//! Cached snapshots must eventually expose new local usage without a manual refresh.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::json;
use tempfile::TempDir;

use crate::codex_workspaces::{
    CodexLocalProjectUsageSnapshot, CodexWorkspacesIndex, ProgressPhase, WorkspaceUsageSidecar,
};
use crate::cost_reporting_period::cost_bucket_zone;

struct Fixture {
    _root: TempDir,
    home: PathBuf,
    index: CodexWorkspacesIndex,
    sidecar: WorkspaceUsageSidecar,
    now: DateTime<Utc>,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let home = root.path().join("codex");
        fs::create_dir_all(&home).unwrap();
        // Keep thread-name lookup inside the synthetic home as well.
        fs::write(
            home.join("config.toml"),
            format!(
                "sqlite_home = '{}'\n",
                home.canonicalize().unwrap().display()
            ),
        )
        .unwrap();
        let sidecar_path = root.path().join("workspaces.sqlite");
        let index = CodexWorkspacesIndex::new(30)
            .with_codex_home(&home)
            .with_sidecar_path(&sidecar_path);
        Self {
            _root: root,
            home,
            index,
            sidecar: WorkspaceUsageSidecar::new(sidecar_path),
            now: Utc::now(),
        }
    }

    fn write_session(&self, id: &str, input: u64, output: u64) -> PathBuf {
        let folder = self
            .home
            .join("sessions")
            .join(self.now.format("%Y/%m/%d").to_string());
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(format!("{id}.jsonl"));
        let rows = [
            json!({
                "timestamp": self.now.to_rfc3339(),
                "type": "session_meta",
                "payload": {"session_id": id, "cwd": self.home, "source": "cli"}
            }),
            json!({
                "timestamp": self.now.to_rfc3339(),
                "type": "turn_context",
                "payload": {"model": "gpt-5"}
            }),
        ];
        fs::write(&path, format!("{}\n{}\n", rows[0], rows[1])).unwrap();
        self.append_usage(&path, input, output);
        path
    }

    fn append_usage(&self, path: &Path, input: u64, output: u64) {
        let row = json!({
            "timestamp": self.now.to_rfc3339(),
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {"last_token_usage": {
                    "input_tokens": input,
                    "cached_input_tokens": 0,
                    "output_tokens": output
                }}
            }
        });
        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        writeln!(file, "{row}").unwrap();
    }

    fn cache_at(&self, updated_at: DateTime<Utc>) -> CodexLocalProjectUsageSnapshot {
        let mut snapshot = self.index.load_snapshot(false, |_| {}).unwrap();
        assert_eq!(snapshot.total.total_tokens, 110);
        snapshot.updated_at = updated_at;
        self.sidecar.publish_snapshot(&snapshot).unwrap();
        snapshot
    }

    fn assert_refreshed_today(&self, expected_sessions: usize) {
        let mut phases = Vec::new();
        let snapshot = self
            .index
            .load_snapshot(false, |progress| phases.push(progress.phase))
            .unwrap();
        assert_eq!(snapshot.total.total_tokens, 330);
        assert_eq!(snapshot.sessions.len(), expected_sessions);
        let today = cost_bucket_zone().date(self.now).to_string();
        assert_eq!(
            snapshot
                .daily
                .iter()
                .find(|point| point.day == today)
                .unwrap()
                .total_tokens,
            330
        );
        assert!(phases.contains(&ProgressPhase::ScanningLogs));
        let persisted = self.index.load_cached_snapshot().unwrap().unwrap();
        assert_eq!(persisted.total, snapshot.total);
        assert_eq!(persisted.updated_at, snapshot.updated_at);
    }
}

#[test]
fn expired_snapshot_discovers_new_today_session_without_forced_refresh() {
    let fixture = Fixture::new();
    fixture.write_session("existing-session", 100, 10);
    fixture.cache_at(fixture.now - Duration::minutes(6));
    fixture.write_session("new-today-session", 200, 20);

    fixture.assert_refreshed_today(2);
}

#[test]
fn expired_snapshot_reads_appended_today_usage_without_forced_refresh() {
    let fixture = Fixture::new();
    let path = fixture.write_session("existing-session", 100, 10);
    fixture.cache_at(fixture.now - Duration::minutes(6));
    fixture.append_usage(&path, 200, 20);

    fixture.assert_refreshed_today(1);
}

#[test]
fn fresh_snapshot_is_reused_until_its_refresh_interval_expires() {
    let fixture = Fixture::new();
    fixture.write_session("existing-session", 100, 10);
    let cached = fixture.cache_at(fixture.now);
    fixture.write_session("new-today-session", 200, 20);

    let mut phases = Vec::new();
    let snapshot = fixture
        .index
        .load_snapshot(false, |progress| phases.push(progress.phase))
        .unwrap();

    assert_eq!(snapshot.total, cached.total);
    assert_eq!(snapshot.sessions, cached.sessions);
    assert_eq!(snapshot.updated_at, cached.updated_at);
    assert!(phases.is_empty());
}

#[test]
fn future_snapshot_is_refreshed_after_clock_rollback() {
    let fixture = Fixture::new();
    fixture.write_session("existing-session", 100, 10);
    fixture.cache_at(fixture.now + Duration::minutes(10));
    fixture.write_session("new-today-session", 200, 20);

    fixture.assert_refreshed_today(2);
}

#[test]
fn crossing_reporting_midnight_refreshes_snapshot_before_five_minutes() {
    let mut fixture = Fixture::new();
    let zone = cost_bucket_zone();
    let today = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
    let midnight = zone.start_of_day_utc(today);
    let before_midnight = midnight - Duration::minutes(1);
    let after_midnight = midnight + Duration::minutes(1);
    assert_ne!(zone.date(before_midnight), zone.date(after_midnight));
    assert_eq!((after_midnight - before_midnight).num_seconds(), 120);

    fixture.index = CodexWorkspacesIndex::new(1)
        .with_codex_home(&fixture.home)
        .with_sidecar_path(fixture.sidecar.path());
    fixture.now = before_midnight;
    fixture.write_session("yesterday-session", 100, 10);
    let cached = fixture
        .index
        .load_snapshot_at(false, |_| {}, before_midnight)
        .unwrap();
    assert_eq!(cached.total.total_tokens, 110);

    fixture.now = after_midnight;
    fixture.write_session("today-session", 200, 20);
    let mut phases = Vec::new();
    let snapshot = fixture
        .index
        .load_snapshot_at(
            false,
            |progress| phases.push(progress.phase),
            after_midnight,
        )
        .unwrap();

    assert_eq!(snapshot.total.total_tokens, 220);
    assert_eq!(snapshot.daily.len(), 1);
    assert_eq!(snapshot.daily[0].day, today.to_string());
    assert_eq!(snapshot.daily[0].total_tokens, 220);
    assert_eq!(snapshot.updated_at, after_midnight);
    assert!(phases.contains(&ProgressPhase::ScanningLogs));
}
