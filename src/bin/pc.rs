use anyhow::Result;
use chrono::{Local, NaiveDate};
use clap::Parser;
use paychecker::{db, tui};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pc",
    about = "Paychecker: record paychecks and see where each one goes"
)]
struct Cli {
    /// Database file. Defaults to ~/.local/share/paychecker/paychecks.db
    #[arg(long)]
    db: Option<PathBuf>,
    /// Treat this date as today. Defaults to the local date.
    #[arg(long)]
    today: Option<NaiveDate>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.db {
        Some(path) => path,
        None => db::default_path()?,
    };
    let db = db::open(&path)?;
    let today = cli.today.unwrap_or_else(|| Local::now().date_naive());
    tui::run(db, today)
}
