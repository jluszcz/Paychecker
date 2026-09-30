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
