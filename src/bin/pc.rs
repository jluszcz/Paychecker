use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use jluszcz_finance_utils::backup::cli::{self as backup, BackupArgs};
use jluszcz_finance_utils::cli::CommonArgs;
use paychecker::{BACKUP, config, db, report, tui};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pc",
    about = "Paychecker: record paychecks and see where each one goes",
    mut_arg("db", |a| a.help("Database file. Defaults to ~/.local/share/paychecker/paychecks.db")),
    mut_arg("scratch", |a| a.help(
        "Run against a copy of the default database in a fresh temporary \
         directory, leaving the real one untouched -- for trying a migration \
         before it reaches the file that matters. The copy is left behind and \
         its path printed, so it can be inspected afterwards, and the report is \
         written beside it rather than into the configured directory"
    )),
    mut_arg("config", |a| a.help("Config file. Defaults to ~/.config/paychecker/config.toml"))
)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
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
    Backup(BackupArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.common.config_path(config::APP)?;
    // Before the TUI opens: a config that does not parse should say so on a
    // terminal in its normal mode, not after a session's work. `pc report
    // --dir` reads nothing from it, so a broken file does not stop that run.
    let cfg = match &cli.command {
        Some(Command::Report { dir: Some(_) }) => config::Config::default(),
        _ => config::load(&config_path)?,
    };
    let is_explicit_backup = matches!(cli.command, Some(Command::Backup(_)));
    // Refused before the copy is made: a throwaway copy has nothing worth
    // restoring, and an upload of one would sit in the bucket beside the real
    // backups looking like one.
    if cli.common.scratch && is_explicit_backup {
        anyhow::bail!("--scratch cannot be backed up: drop the flag to back up the real database");
    }
    // A run pointed at another database or another day is a scratch session;
    // see `report::write_if_enabled`.
    let scratch = cli.common.is_scratch_session();
    // The schedule belongs to the default database only, and a `--scratch`
    // copy is no more it than a `--db` is.
    let is_default_db = cli.common.is_default_db();
    // `db::snapshot` opens nothing through `db::open`, so a scratch copy keeps
    // the schema version the original has and this run is the one that
    // migrates it.
    let path = cli
        .common
        .db_path(BACKUP.app, db::default_path, db::snapshot)?;
    if cli.common.scratch {
        eprintln!("scratch database: {}", path.display());
    }
    // The copy's own directory: a `--scratch` run's page goes there, beside
    // the database it was rendered from, never over the real one.
    let scratch_dir = path
        .parent()
        .filter(|_| cli.common.scratch)
        .map(PathBuf::from);
    let today = cli.common.today_or_local();

    match cli.command {
        None => {
            let db = tui::run(db::open(&path)?, today)?;
            // The session's work is already saved, so a report that cannot be
            // written is a warning, not a failed run. A scratch directory's
            // page is written whatever the config says: it is fresh, and the
            // page is there to be compared with the real one.
            let outcome = match &scratch_dir {
                Some(dir) => report::write(&db, dir, today).map(report::Outcome::Written),
                None => report::write_if_enabled(&db, &cfg, today, scratch),
            };
            match outcome {
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
            let dir = match dir.or(scratch_dir) {
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
        Some(Command::Backup(args)) => {
            backup::command(&BACKUP, &path, cfg.backup.as_ref(), &args, db::snapshot)?
        }
    }

    // The state file records when an upload last happened, not what was
    // uploaded, so a `--db` copy on the schedule would take the real
    // database's turn. An explicit `pc backup` uploads whatever it is given.
    if !is_explicit_backup && is_default_db {
        backup::scheduled(&BACKUP, &path, cfg.backup.as_ref(), db::snapshot);
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
