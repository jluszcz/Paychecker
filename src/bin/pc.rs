use anyhow::Result;
use clap::{Parser, Subcommand};
use jluszcz_finance_utils::backup::cli::{self as backup, BackupArgs};
use jluszcz_finance_utils::cli::CommonArgs;
use jluszcz_finance_utils::report::cli::{self as report_cli, ReportArgs};
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
    Report(ReportArgs),
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
        Some(Command::Report(ReportArgs { dir: Some(_) })) => config::Config::default(),
        _ => config::load(&config_path)?,
    };
    let is_explicit_backup = matches!(cli.command, Some(Command::Backup(_)));
    if is_explicit_backup {
        cli.common.refuse_scratch_backup()?;
    }
    // A run pointed at another database or another day is a scratch session;
    // see `report::write_if_enabled`.
    let scratch = cli.common.is_scratch_session();
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
            report_cli::after_quit(match &scratch_dir {
                Some(dir) => report::write(&db, dir, today).map(report::Outcome::Written),
                None => report::write_if_enabled(&db, &cfg, today, scratch),
            });
        }
        Some(Command::Report(args)) => {
            let db = db::open(&path)?;
            let dir = report_cli::dir(
                &args,
                scratch_dir.as_deref(),
                cfg.report.as_ref(),
                &config_path,
            )?;
            println!(
                "{}",
                report_cli::describe(&report::write(&db, &dir, today)?)
            );
        }
        // Never opens the database: opening creates and seeds a missing
        // file, and a mistyped `--db` would then be uploaded as a backup.
        Some(Command::Backup(args)) => {
            backup::command(&BACKUP, &path, cfg.backup.as_ref(), &args, db::snapshot)?
        }
    }

    if !is_explicit_backup {
        cli.common
            .scheduled_backup(&BACKUP, &path, cfg.backup.as_ref(), db::snapshot);
    }
    Ok(())
}
