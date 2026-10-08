//! `pc backup` end to end, against scratch paths and an AWS environment that
//! can reach nothing.

use jluszcz_finance_utils::testing::unreachable_aws;
use std::path::PathBuf;
use std::process::Command;

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "paychecker_backup_cli_{label}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A mistyped `--db` must not become a freshly created, seeded database that
/// is uploaded as a restore point and stamped as the latest backup.
#[test]
fn backing_up_a_database_path_that_does_not_exist_creates_nothing() {
    let dir = scratch("missing_db");
    let config = dir.join("config.toml");
    std::fs::write(&config, "[backup]\nbucket = \"a-bucket\"\n").unwrap();
    let db = dir.join("typo.db");

    let output = unreachable_aws(
        Command::new(env!("CARGO_BIN_EXE_pc"))
            .args([
                "--db",
                db.to_str().unwrap(),
                "--config",
                config.to_str().unwrap(),
            ])
            .args(["backup", "--force"])
            .env("XDG_STATE_HOME", dir.join("state")),
    )
    .output()
    .unwrap();

    assert!(!output.status.success());
    assert!(!db.exists(), "pc backup created {}", db.display());
    assert!(!dir.join("state").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_help_names_the_default_database_and_config_paths() {
    let output = Command::new(env!("CARGO_BIN_EXE_pc"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("~/.local/share/paychecker/paychecks.db"),
        "{help}"
    );
    assert!(help.contains("~/.config/paychecker/config.toml"), "{help}");
}
