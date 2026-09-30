//! Paychecker: record paychecks and see where each one goes.

pub mod calc;
pub mod config;
pub mod db;
pub mod money;
pub mod report;
pub mod tui;

/// Paychecker's backups: named `paychecks-<timestamp>.db`, as the
/// `paychecker` profile, with the state at `$XDG_STATE_HOME/paychecker/`.
pub const BACKUP: jluszcz_finance_utils::backup::Spec = jluszcz_finance_utils::backup::Spec {
    app: config::APP,
    stem: "paychecks",
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use jluszcz_finance_utils::config::BackupConfig;

    #[test]
    fn backup_key_is_the_paychecks_stem_and_a_utc_timestamp() {
        let now = Utc.with_ymd_and_hms(2026, 8, 20, 14, 3, 5).unwrap();
        assert_eq!(BACKUP.key_for(now), "paychecks-20260820T140305Z.db.zst");
    }

    #[test]
    fn backup_app_name_is_paychecker() {
        assert_eq!(BACKUP.app, "paychecker");
    }

    #[test]
    fn backup_profile_defaults_to_the_app_name_when_unset() {
        let config = BackupConfig {
            bucket: "a-bucket".into(),
            profile: None,
            interval_days: 7,
        };
        assert_eq!(config.profile_or(BACKUP.app), "paychecker");
    }
}
