//! `skilldock` (alias `sd`) — a thin CLI adapter over `skilldock-core`.
//!
//! It resolves the dock from the environment, calls one core operation, and
//! renders the result. No business logic lives here; both the `skilldock` and
//! `sd` binaries call [`run`].

use anyhow::Result;
use clap::{Parser, Subcommand};
use skilldock_core::{self as core, Skilldock};

#[derive(Parser)]
#[command(
    name = "skilldock",
    version,
    about = "Manage your Agent Skills: authored originals and vendored dependencies."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List skills by provenance (vendored / authored).
    List {
        /// Emit structured JSON instead of a human-readable table.
        #[arg(long)]
        json: bool,
    },
    /// Mark or scaffold an authored skill and record it in the manifest.
    Author {
        /// The skill name (a single directory-name component).
        name: String,
    },
}

/// Parse arguments and run the selected command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let sd = Skilldock::from_env()?;

    match cli.command {
        Command::List { json } => run_list(&sd, json)?,
        Command::Author { name } => run_author(&sd, &name)?,
    }
    Ok(())
}

fn run_list(sd: &Skilldock, json: bool) -> Result<()> {
    let listing = core::list(sd)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&listing)?);
        return Ok(());
    }

    println!("authored ({}):", listing.authored.len());
    for s in &listing.authored {
        let flag = if s.present { "" } else { "  (missing)" };
        println!("  {}{}", s.name, flag);
    }
    println!("vendored ({}):", listing.vendored.len());
    for s in &listing.vendored {
        println!("  {}  {}  {}", s.name, s.repo, &s.resolved);
    }
    Ok(())
}

fn run_author(sd: &Skilldock, name: &str) -> Result<()> {
    let outcome = core::author(sd, name)?;
    let what = if outcome.scaffolded {
        "scaffolded"
    } else if outcome.already_listed {
        "already tracked"
    } else {
        "marked"
    };
    println!("{} {}", what, outcome.name);
    Ok(())
}
