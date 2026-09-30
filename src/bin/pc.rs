use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand};
use paychecker::{config, db, report, tui};
use std::path::PathBuf;

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
    let path = match cli.db {
        Some(path) => path,
        None => db::default_path()?,
    };
    let db = db::open(&path)?;
    let today = cli.today.unwrap_or_else(|| Local::now().date_naive());

    match cli.command {
        None => {
            let db = tui::run(db, today)?;
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
    }
    Ok(())
}

fn print_written(written: &report::Written) {
    println!(
        "wrote {} to {}",
        report::human_bytes(written.bytes),
        written.path.display()
    );
}
