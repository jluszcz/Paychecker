//! Backing the database up to S3: the schedule, the snapshot, and the run
//! that joins them to the upload.

pub mod s3;
pub mod state;

use crate::config::Config;
use crate::db;
use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use std::path::Path;

/// Whether a backup is owed, against the real clock rather than `--today`:
/// `--today` simulates a pay date, and whether a file reached S3 is a fact
/// about wall time.
pub fn is_due(last: Option<DateTime<Utc>>, now: DateTime<Utc>, interval_days: u32) -> bool {
    match last {
        None => true,
        Some(last) => now.signed_duration_since(last) >= interval(interval_days),
    }
}

pub fn next_due(last: DateTime<Utc>, interval_days: u32) -> DateTime<Utc> {
    last + interval(interval_days)
}

/// Ten years. `interval_days` comes out of a hand-edited file, and
/// `DateTime`'s addition panics once the sum leaves chrono's calendar; past a
/// decade the setting is a typo rather than a schedule.
const MAX_INTERVAL_DAYS: u32 = 3653;

fn interval(days: u32) -> TimeDelta {
    TimeDelta::days(i64::from(days.min(MAX_INTERVAL_DAYS)))
}

// The snapshot is the whole database in cleartext, in a directory under `/tmp`
// on Linux. The leaf is created non-recursively with mode 0700, and that is
// the guard: a recursive create returns `Ok` for a directory someone else made
// first, keeps its mode, and follows a symlink planted there. A directory
// already at this path (a killed run that had the same pid) is removed first;
// anything that survives the removal is left for the `create` to fail on. A
// run killed mid-upload leaves its own pid's directory behind, mode 0700,
// for the system's temp cleaner.
#[cfg(unix)]
fn create_snapshot_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::fs::DirBuilder::new().mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn create_snapshot_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// At the root of the bucket, under no prefix: the bucket holds nothing else,
/// so a prefix would be a second spelling the IAM policy has to match.
pub fn key_for(now: DateTime<Utc>) -> String {
    format!("paychecks-{}.db", now.format("%Y%m%dT%H%M%SZ"))
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// No `[backup]` section: the feature is off.
    Disabled,
    NotDue {
        next: DateTime<Utc>,
    },
    BackedUp {
        bucket: String,
        key: String,
        bytes: u64,
    },
}

/// Snapshot, upload, and record, when the schedule says so or `force`
/// overrides it.
pub fn run_if_due(
    db_path: &Path,
    cfg: &Config,
    state_path: &Path,
    now: DateTime<Utc>,
    force: bool,
) -> Result<Outcome> {
    // Named for the process, so two `pc` runs cannot share a snapshot path.
    let dir = std::env::temp_dir().join(format!("paychecker-backup-{}", std::process::id()));
    run(db_path, cfg, state_path, now, force, &dir, s3::upload)
}

/// `run_if_due` with the snapshot directory and the upload passed in, so a
/// test runs the whole path without S3 and without sharing a directory with a
/// test beside it.
fn run(
    db_path: &Path,
    cfg: &Config,
    state_path: &Path,
    now: DateTime<Utc>,
    force: bool,
    dir: &Path,
    upload: impl FnOnce(&str, &str, &str, &Path) -> Result<()>,
) -> Result<Outcome> {
    let Some(backup) = cfg.backup.as_ref() else {
        return Ok(Outcome::Disabled);
    };

    let last = match state::read(state_path) {
        Ok(state) => state.map(|s| s.last_backup_at),
        // A warning rather than a refusal: being wrong costs one redundant
        // upload, after which the file is rewritten and correct again.
        Err(e) => {
            eprintln!("ignoring unreadable backup state: {e:#}");
            None
        }
    };

    if !force
        && let Some(last) = last
        && !is_due(Some(last), now, backup.interval_days)
    {
        return Ok(Outcome::NotDue {
            next: next_due(last, backup.interval_days),
        });
    }

    create_snapshot_dir(dir).with_context(|| format!("creating {}", dir.display()))?;
    let snapshot = dir.join("paychecks.db");
    let key = key_for(now);
    let result = (|| {
        db::snapshot(db_path, &snapshot)?;
        let bytes = std::fs::metadata(&snapshot)
            .with_context(|| format!("measuring {}", snapshot.display()))?
            .len();
        upload(
            backup.profile_or(crate::config::APP),
            &backup.bucket,
            &key,
            &snapshot,
        )?;
        Ok::<u64, anyhow::Error>(bytes)
    })();

    // On both paths: a copy of the database must not outlive a failed upload
    // in the temp directory.
    let _ = std::fs::remove_dir_all(dir);
    let bytes = result?;

    state::write(
        state_path,
        &state::State {
            last_backup_at: now,
            last_key: key.clone(),
        },
    )?;

    Ok(Outcome::BackedUp {
        bucket: backup.bucket.clone(),
        key,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, day, hour, 0, 0).unwrap()
    }

    /// A fresh directory named for the test, so tests running at once in one
    /// process never share one.
    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("paychecker_backup_{label}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn a_database(dir: &Path) -> PathBuf {
        let path = dir.join("paychecks.db");
        db::open(&path).unwrap();
        path
    }

    #[test]
    fn a_database_that_has_never_been_backed_up_is_due() {
        assert!(is_due(None, at(20, 0), 7));
    }

    #[test]
    fn a_backup_one_day_short_of_the_interval_is_not_due() {
        assert!(!is_due(Some(at(14, 0)), at(20, 0), 7));
    }

    #[test]
    fn a_backup_exactly_the_interval_old_is_due() {
        assert!(is_due(Some(at(13, 0)), at(20, 0), 7));
    }

    /// A clock that went backwards: not due, and healed once the clock passes
    /// the recorded time.
    #[test]
    fn a_last_backup_in_the_future_is_not_due() {
        assert!(!is_due(Some(at(25, 0)), at(20, 0), 7));
    }

    #[test]
    fn an_interval_of_zero_days_is_always_due() {
        assert!(is_due(Some(at(20, 0)), at(20, 0), 0));
    }

    #[test]
    fn the_next_backup_is_due_one_interval_after_the_last() {
        assert_eq!(next_due(at(13, 0), 7), at(20, 0));
    }

    #[test]
    fn an_absurd_interval_is_clamped_rather_than_panicking() {
        assert_eq!(
            next_due(at(20, 0), u32::MAX),
            at(20, 0) + TimeDelta::days(3653)
        );
    }

    /// Sortable, so `aws s3 ls` lists the history in order.
    #[test]
    fn a_key_is_a_sortable_utc_timestamp_under_no_prefix() {
        let now = Utc.with_ymd_and_hms(2026, 8, 20, 14, 3, 5).unwrap();
        assert_eq!(key_for(now), "paychecks-20260820T140305Z.db");
    }

    #[cfg(unix)]
    #[test]
    fn a_snapshot_directory_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let dir = scratch("mode").join("snapshot");
        create_snapshot_dir(&dir).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "mode was {:o}", mode & 0o777);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// The path is predictable, so another local account can create it first.
    /// A recursive create would accept that directory and its mode; this fails
    /// if anyone reaches for `create_dir_all` here.
    #[cfg(unix)]
    #[test]
    fn a_snapshot_directory_that_already_exists_does_not_keep_its_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = scratch("squatted").join("snapshot");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        std::fs::write(dir.join("planted"), b"planted").unwrap();

        create_snapshot_dir(&dir).unwrap();

        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "mode was {:o}", mode & 0o777);
        assert!(!dir.join("planted").exists());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_config_with_no_backup_section_does_nothing() {
        let outcome = run_if_due(
            Path::new("/nonexistent/paychecks.db"),
            &Config::default(),
            Path::new("/nonexistent/backup.toml"),
            at(20, 0),
            false,
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Disabled);
    }

    /// The database path does not exist: a run that is not due returns before
    /// touching it.
    #[test]
    fn a_recent_backup_returns_when_the_next_is_due_without_reading_the_database() {
        let dir = scratch("not_due");
        let state_path = dir.join("backup.toml");
        state::write(
            &state_path,
            &state::State {
                last_backup_at: at(19, 0),
                last_key: key_for(at(19, 0)),
            },
        )
        .unwrap();

        let outcome = run_if_due(
            Path::new("/nonexistent/paychecks.db"),
            &Config::backing_up_to("a-bucket"),
            &state_path,
            at(20, 0),
            false,
        )
        .unwrap();
        assert_eq!(outcome, Outcome::NotDue { next: at(26, 0) });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_due_backup_uploads_the_snapshot_and_records_when() {
        let dir = scratch("due");
        let db_path = a_database(&dir);
        let state_path = dir.join("backup.toml");
        let snapshot_dir = dir.join("snapshot");
        let mut uploaded = None;

        let outcome = run(
            &db_path,
            &Config::backing_up_to("a-bucket"),
            &state_path,
            at(20, 0),
            false,
            &snapshot_dir,
            |profile, bucket, key, file| {
                uploaded = Some((
                    profile.to_string(),
                    bucket.to_string(),
                    key.to_string(),
                    std::fs::metadata(file)?.len(),
                ));
                Ok(())
            },
        )
        .unwrap();

        let key = key_for(at(20, 0));
        let (profile, bucket, uploaded_key, bytes) = uploaded.unwrap();
        assert_eq!(profile, "a-profile");
        assert_eq!(bucket, "a-bucket");
        assert_eq!(uploaded_key, key);
        assert!(bytes > 0);
        assert_eq!(
            outcome,
            Outcome::BackedUp {
                bucket: "a-bucket".to_string(),
                key: key.clone(),
                bytes,
            }
        );
        assert_eq!(
            state::read(&state_path).unwrap(),
            Some(state::State {
                last_backup_at: at(20, 0),
                last_key: key,
            })
        );
        assert!(!snapshot_dir.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_upload_leaves_no_snapshot_and_keeps_the_backup_due() {
        let dir = scratch("failed");
        let db_path = a_database(&dir);
        let state_path = dir.join("backup.toml");
        let snapshot_dir = dir.join("snapshot");

        let result = run(
            &db_path,
            &Config::backing_up_to("a-bucket"),
            &state_path,
            at(20, 0),
            false,
            &snapshot_dir,
            |_, _, _, _| Err(anyhow::anyhow!("the network is down")),
        );

        assert!(result.is_err());
        assert!(!snapshot_dir.exists());
        assert_eq!(state::read(&state_path).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_state_file_is_treated_as_never_backed_up() {
        let dir = scratch("unreadable");
        let db_path = a_database(&dir);
        let state_path = dir.join("backup.toml");
        std::fs::write(&state_path, "this is not toml {{{").unwrap();

        let outcome = run(
            &db_path,
            &Config::backing_up_to("a-bucket"),
            &state_path,
            at(20, 0),
            false,
            &dir.join("snapshot"),
            |_, _, _, _| Ok(()),
        )
        .unwrap();

        assert!(matches!(outcome, Outcome::BackedUp { .. }));
        assert!(state::read(&state_path).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn force_uploads_a_backup_that_is_not_due() {
        let dir = scratch("force");
        let db_path = a_database(&dir);
        let state_path = dir.join("backup.toml");
        state::write(
            &state_path,
            &state::State {
                last_backup_at: at(19, 0),
                last_key: key_for(at(19, 0)),
            },
        )
        .unwrap();

        let outcome = run(
            &db_path,
            &Config::backing_up_to("a-bucket"),
            &state_path,
            at(20, 0),
            true,
            &dir.join("snapshot"),
            |_, _, _, _| Ok(()),
        )
        .unwrap();

        assert!(matches!(outcome, Outcome::BackedUp { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
