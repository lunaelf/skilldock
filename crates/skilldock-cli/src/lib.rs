//! `skilldock` (alias `sd`) — a thin CLI adapter over `skilldock-core`.
//!
//! It resolves the dock from the environment, calls one core operation, and
//! renders the result. No business logic lives here; both the `skilldock` and
//! `sd` binaries call [`run`].

use anyhow::Result;
use clap::{Parser, Subcommand};
use skilldock_core::{self as core, AddRequest, SkillSpec, Skilldock};

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
    /// Declare a vendored source, clone it into the Cache, and pin it.
    Add {
        /// Repo: `owner/repo`, `host/owner/repo`, or a git URL.
        repo: String,
        /// One or more skill subpaths or globs to vendor from the repo.
        #[arg(required = true)]
        skills: Vec<String>,
        /// Branch or tag to pin (defaults to the repo's default branch).
        #[arg(long = "ref")]
        git_ref: Option<String>,
    },
    /// Reconstruct the Cache to exactly match the lock.
    Sync,
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
        Command::Add {
            repo,
            skills,
            git_ref,
        } => run_add(&sd, &repo, skills, git_ref)?,
        Command::Sync => run_sync(&sd)?,
        Command::List { json } => run_list(&sd, json)?,
        Command::Author { name } => run_author(&sd, &name)?,
    }
    Ok(())
}

fn run_add(sd: &Skilldock, repo: &str, skills: Vec<String>, git_ref: Option<String>) -> Result<()> {
    let outcome = core::add(
        sd,
        AddRequest {
            source: core::parse_source(repo)?,
            git_ref,
            skills: skills.into_iter().map(SkillSpec::Path).collect(),
        },
    )?;
    println!(
        "added {} @ {} ({} skill{})",
        outcome.repo,
        &outcome.resolved[..outcome.resolved.len().min(12)],
        outcome.skills.len(),
        if outcome.skills.len() == 1 { "" } else { "s" }
    );
    for s in &outcome.skills {
        println!("  {}  {}", s.name, s.path);
    }
    Ok(())
}

fn run_sync(sd: &Skilldock) -> Result<()> {
    let outcome = core::sync(sd)?;
    println!(
        "sync: {} cloned, {} updated",
        outcome.cloned.len(),
        outcome.updated.len()
    );
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
