use anyhow::{Context, Result};
use chrono::{Local, NaiveDate, Utc};
use clap::{Parser, Subcommand};
use paychecker::{backup, config, db, report, tui};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "pc",
    about = "Paychecker: record paychecks and see where each one goes"
)]
struct Cli {
    /// Database file. Defaults to ~/.local/share/paychecker/paychecks.db
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    /// Treat this date as today. Defaults to the local date.
    #[arg(long, global = true)]
    today: Option<NaiveDate>,
    /// Config file. Defaults to ~/.config/paychecker/config.toml
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Write the HTML report without opening the application.
    Report {
        /// Directory to write Paychecks.html into. Defaults to the config
        /// file's [report] dir, which this makes optional.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Back the database up to S3, if the schedule says one is due.
    Backup {
        /// Upload even if the last backup is recent enough.
        #[arg(long)]
        force: bool,
        /// Print when the last backup ran and when the next is due, then exit.
        #[arg(long, conflicts_with = "force")]
        status: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = match cli.config {
        Some(path) => path,
        None => config::default_path()?,
    };
    // Before the TUI opens: a config that does not parse should say so on a
    // terminal in its normal mode, not after a session's work. `pc report
    // --dir` reads nothing from it, so a broken file does not stop that run.
    let cfg = match &cli.command {
        Some(Command::Report { dir: Some(_) }) => config::Config::default(),
        _ => config::load(&config_path)?,
    };
    // A run pointed at another database or another day is a scratch session;
    // see `report::write_if_enabled`.
    let scratch = cli.db.is_some() || cli.today.is_some();
    // Asked before `cli.db` is moved into a path: the schedule belongs to the
    // default database only.
    let is_default_db = cli.db.is_none();
    let path = match cli.db {
        Some(path) => path,
        None => db::default_path()?,
    };
    let today = cli.today.unwrap_or_else(|| Local::now().date_naive());
    let is_explicit_backup = matches!(cli.command, Some(Command::Backup { .. }));

    match cli.command {
        None => {
            let db = tui::run(db::open(&path)?, today)?;
            // The session's work is already saved, so a report that cannot be
            // written is a warning, not a failed run.
            match report::write_if_enabled(&db, &cfg, today, scratch) {
                Ok(report::Outcome::Written(written)) => print_written(&written),
                // Silent: nothing happened, and this runs after every quit.
                Ok(_) => {}
                Err(e) => eprintln!("report failed: {e:#}"),
            }
        }
        Some(Command::Report { dir }) => {
            let db = db::open(&path)?;
            // An unset [report] section means "not on every quit", which is
            // a different question from the one `pc report` asks.
            let dir = match dir {
                Some(dir) => dir,
                None => cfg
                    .report
                    .as_ref()
                    .with_context(|| {
                        format!(
                            "no --dir given, and no [report] section naming one in {}",
                            config_path.display()
                        )
                    })?
                    .dir()?,
            };
            print_written(&report::write(&db, &dir, today)?);
        }
        // Never opens the database: opening creates and seeds a missing
        // file, and a mistyped `--db` would then be uploaded as a backup.
        Some(Command::Backup { force, status }) => {
            let state_path = backup::state::default_path()?;
            if status {
                print_backup_status(&cfg, &state_path);
            } else {
                // Asked for outright, so a failure is an error exit rather
                // than a line on stderr.
                let outcome = backup::run_if_due(&path, &cfg, &state_path, Utc::now(), force)?;
                print_backup(&outcome);
            }
        }
    }

    // The state file records when an upload last happened, not what was
    // uploaded, so a `--db` copy on the schedule would take the real
    // database's turn. An explicit `pc backup` uploads whatever it is given.
    if !is_explicit_backup && is_default_db {
        scheduled_backup(&path, &cfg);
    }
    Ok(())
}

fn print_written(written: &report::Written) {
    println!(
        "wrote {} to {}",
        jluszcz_finance_utils::human_bytes(written.bytes),
        written.path.display()
    );
}

const BACKUP_TIME: &str = "%Y-%m-%d %H:%M UTC";

/// Never fatal: the session's work is saved, and a failed upload leaves the
/// schedule due, so the next run tries again.
fn scheduled_backup(db_path: &Path, cfg: &config::Config) {
    // The state path needs `HOME` or `XDG_STATE_HOME`, so it is resolved only
    // once there is a backup to schedule.
    if cfg.backup.is_none() {
        return;
    }
    let result = backup::state::default_path()
        .and_then(|state_path| backup::run_if_due(db_path, cfg, &state_path, Utc::now(), false));
    match result {
        Ok(outcome @ backup::Outcome::BackedUp { .. }) => print_backup(&outcome),
        // Silent: nothing happened, and this runs after every quit.
        Ok(_) => {}
        Err(e) => eprintln!("backup failed: {e:#}"),
    }
}

fn print_backup(outcome: &backup::Outcome) {
    match outcome {
        backup::Outcome::Disabled => {
            println!("backups are off: no [backup] section in the config file");
        }
        backup::Outcome::NotDue { next } => {
            println!("not due until {}", next.format(BACKUP_TIME));
        }
        backup::Outcome::BackedUp { bucket, key, bytes } => {
            println!(
                "backed up {} to s3://{bucket}/{key}",
                jluszcz_finance_utils::human_bytes(*bytes)
            );
        }
    }
}

/// An unreadable state file warns rather than failing: `--status` must not be
/// stricter than the scheduled check it reports on.
fn print_backup_status(cfg: &config::Config, state_path: &Path) {
    let Some(backup_cfg) = cfg.backup.as_ref() else {
        println!("backups are off: no [backup] section in the config file");
        return;
    };
    println!(
        "bucket {}, profile {}, every {} days",
        backup_cfg.bucket,
        backup_cfg.profile_or(config::APP),
        backup_cfg.interval_days
    );
    let state = match backup::state::read(state_path) {
        Ok(state) => state,
        Err(e) => {
            eprintln!("ignoring unreadable backup state: {e:#}");
            None
        }
    };
    match state {
        None => println!("never backed up"),
        Some(state) => {
            println!(
                "last {} ({})",
                state.last_backup_at.format(BACKUP_TIME),
                state.last_key
            );
            println!(
                "next {}",
                backup::next_due(state.last_backup_at, backup_cfg.interval_days)
                    .format(BACKUP_TIME)
            );
        }
    }
}
