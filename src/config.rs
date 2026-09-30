//! The configuration file.
//!
//! An absent file, or one missing a section, means that section's feature is
//! off: a clean checkout and an unconfigured machine both do nothing. A file
//! that is present but does not parse is an error instead. `[report]`'s `dir`
//! and `[backup]`'s `bucket` have no default, so the typo that would otherwise
//! switch a feature off silently (`directory =`, `bucketname =`) is a missing
//! field. Keys nothing reads are ignored.

use anyhow::Result;
use jluszcz_finance_utils::config::{self as shared, BackupConfig, ReportConfig};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const APP: &str = "paychecker";

#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Config {
    pub report: Option<ReportConfig>,
    pub backup: Option<BackupConfig>,
}

impl Config {
    #[cfg(test)]
    pub(crate) fn reporting_to(dir: &Path) -> Config {
        Config {
            report: Some(ReportConfig::new(dir.display().to_string())),
            backup: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn backing_up_to(bucket: &str) -> Config {
        Config {
            report: None,
            backup: Some(BackupConfig {
                bucket: bucket.to_string(),
                profile: Some("a-profile".to_string()),
                interval_days: 7,
            }),
        }
    }
}

/// `$XDG_CONFIG_HOME/paychecker/config.toml`, or `~/.config` when it is unset
/// or empty.
pub fn default_path() -> Result<PathBuf> {
    shared::default_path(APP)
}

pub fn load(path: &Path) -> Result<Config> {
    shared::load(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `body` to a temp file named for the test, since the tests run in
    /// one process at once and a shared name would have them reading each
    /// other's files.
    fn fixture(label: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "paychecker_config_{label}_{}.toml",
            std::process::id()
        ));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn a_config_file_with_no_report_section_leaves_reports_off() {
        let path = fixture("no_report", "[other]\nkey = 1\n");
        assert_eq!(load(&path).unwrap(), Config::default());
    }

    #[test]
    fn a_config_file_that_does_not_parse_is_an_error_naming_its_path() {
        let path = fixture("broken", "[report\n");
        let err = load(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains(&path.display().to_string()),
            "{err:#}"
        );
    }
}
