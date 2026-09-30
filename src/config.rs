//! The configuration file, and the only place `serde` and `toml` are named.
//!
//! An absent file, or one with no `[report]` section, means the report is off:
//! a clean checkout and an unconfigured machine both do nothing. A file that is
//! present but does not parse is an error instead. `dir` has no default, so the
//! typo that would otherwise switch the report off silently (`directory =`) is
//! a missing field. Keys nothing reads are ignored.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Config {
    pub report: Option<Report>,
}

/// Where the HTML report is written on quit.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Report {
    dir: String,
}

impl Report {
    /// The directory, with a leading `~` or `~/` expanded against `$HOME`.
    /// TOML does not expand it, and a `~` anywhere else is an ordinary
    /// character in a directory name. A relative path is an error: it would
    /// resolve against whichever directory `pc` happened to be started in.
    pub fn dir(&self) -> Result<PathBuf> {
        let home = || std::env::var_os("HOME").context("HOME is not set");
        let dir = if self.dir == "~" {
            PathBuf::from(home()?)
        } else if let Some(rest) = self.dir.strip_prefix("~/") {
            PathBuf::from(home()?).join(rest)
        } else {
            PathBuf::from(&self.dir)
        };
        anyhow::ensure!(
            dir.is_absolute(),
            "[report] dir {:?} is not an absolute path",
            self.dir
        );
        Ok(dir)
    }
}

impl Config {
    #[cfg(test)]
    pub(crate) fn reporting_to(dir: &Path) -> Config {
        Config {
            report: Some(Report {
                dir: dir.display().to_string(),
            }),
        }
    }
}

/// `$XDG_CONFIG_HOME/paychecker/config.toml`, or `~/.config` when it is unset
/// or empty.
pub fn default_path() -> Result<PathBuf> {
    let dir = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let home = std::env::var_os("HOME").context("HOME is not set")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(dir.join("paychecker").join("config.toml"))
}

pub fn load(path: &Path) -> Result<Config> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
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
    fn a_report_section_naming_only_a_dir_is_a_complete_configuration() {
        let path = fixture("report", "[report]\ndir = \"/tmp/reports\"\n");
        let report = load(&path).unwrap().report.unwrap();
        assert_eq!(report.dir().unwrap(), PathBuf::from("/tmp/reports"));
    }

    #[test]
    fn a_misspelled_dir_is_an_error_rather_than_a_silently_disabled_report() {
        let path = fixture("typo", "[report]\ndirectory = \"/tmp/reports\"\n");
        assert!(load(&path).is_err());
    }

    #[test]
    fn a_config_file_with_no_report_section_leaves_reports_off() {
        let path = fixture("no_report", "[other]\nkey = 1\n");
        assert_eq!(load(&path).unwrap(), Config::default());
    }

    #[test]
    fn an_absent_config_file_leaves_reports_off() {
        let path = std::env::temp_dir().join("paychecker_config_absent_nowhere.toml");
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

    #[test]
    fn a_leading_tilde_in_the_report_dir_expands_against_home() {
        let path = fixture("tilde", "[report]\ndir = \"~/reports\"\n");
        let report = load(&path).unwrap().report.unwrap();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(report.dir().unwrap(), home.join("reports"));
    }

    #[test]
    fn a_tilde_in_the_middle_of_the_report_dir_is_left_alone() {
        let path = fixture("mid_tilde", "[report]\ndir = \"/tmp/a~b\"\n");
        let report = load(&path).unwrap().report.unwrap();
        assert_eq!(report.dir().unwrap(), PathBuf::from("/tmp/a~b"));
    }

    #[test]
    fn a_bare_tilde_as_the_report_dir_is_home() {
        let path = fixture("bare_tilde", "[report]\ndir = \"~\"\n");
        let report = load(&path).unwrap().report.unwrap();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(report.dir().unwrap(), home);
    }

    #[test]
    fn a_relative_report_dir_is_an_error_rather_than_a_path_under_wherever_pc_started() {
        let path = fixture("relative", "[report]\ndir = \"Dropbox/pay\"\n");
        let report = load(&path).unwrap().report.unwrap();
        assert!(report.dir().is_err());
    }
}
