//! `skilldock` (alias `sd`) — a thin CLI adapter over `skilldock-core`.
//!
//! It resolves the dock from the environment, calls one core operation, and
//! renders the result. No business logic lives here; both the `skilldock` and
//! `sd` binaries call [`run`].

use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use skilldock_core::{self as core, AddRequest, Consumer, SkillSpec, Skilldock};

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
    /// Remove a vendored skill or repo from the manifest, lock, and Cache.
    #[command(visible_alias = "rm")]
    Remove {
        /// Skill name(s) or repo identity(ies) to remove.
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Re-resolve declared refs to fresh commits and rewrite the lock + Cache.
    Update {
        /// Repo identity(ies) to update; empty updates every declared source.
        repos: Vec<String>,
    },
    /// Reconstruct the Cache to exactly match the lock.
    Sync,
    /// Symlink skills into a Consumer (a project, or `-g` for global).
    Link {
        /// `<project> <skill>...`, or with `-g` just `<skill>...`.
        #[arg(required = true, num_args = 1..)]
        args: Vec<String>,
        /// Install globally into `~/.agents` and `~/.claude`.
        #[arg(short, long)]
        global: bool,
        /// Replace an existing link that points elsewhere.
        #[arg(short, long)]
        force: bool,
    },
    /// Remove skills' links from a Consumer.
    Unlink {
        /// `<project> <skill>...`, or with `-g` just `<skill>...`.
        #[arg(required = true, num_args = 1..)]
        args: Vec<String>,
        /// Operate on the global (`~/.agents` + `~/.claude`) links.
        #[arg(short, long)]
        global: bool,
    },
    /// Remove dangling (broken) links from a Consumer.
    Prune {
        /// The project path; omit with `-g`.
        consumer: Option<String>,
        /// Operate on the global links.
        #[arg(short, long)]
        global: bool,
    },
    /// Re-point a Consumer's links to their current Source paths.
    Relink {
        /// The project path; omit with `-g`.
        consumer: Option<String>,
        /// Operate on the global links.
        #[arg(short, long)]
        global: bool,
    },
    /// Add or remove a project in the `links.txt` registry.
    Register {
        /// Project path(s) to (de)register.
        #[arg(required = true)]
        consumers: Vec<String>,
        /// Deregister instead of registering.
        #[arg(short, long)]
        remove: bool,
    },
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
        Command::Remove { targets } => run_remove(&sd, &targets)?,
        Command::Update { repos } => run_update(&sd, &repos)?,
        Command::Sync => run_sync(&sd)?,
        Command::Link {
            args,
            global,
            force,
        } => {
            let (consumer, skills) = split_consumer_args(global, args)?;
            run_link(&sd, consumer, &skills, force)?
        }
        Command::Unlink { args, global } => {
            let (consumer, skills) = split_consumer_args(global, args)?;
            run_unlink(&sd, consumer, &skills)?
        }
        Command::Prune { consumer, global } => run_prune(&sd, make_consumer(global, consumer)?)?,
        Command::Relink { consumer, global } => run_relink(&sd, make_consumer(global, consumer)?)?,
        Command::Register { consumers, remove } => run_register(&sd, &consumers, remove)?,
        Command::List { json } => run_list(&sd, json)?,
        Command::Author { name } => run_author(&sd, &name)?,
    }
    Ok(())
}

/// The global consumer, rooted at the user's home (`~/.agents` + `~/.claude`).
fn global_consumer() -> Result<Consumer> {
    let home = dirs::home_dir().context("could not locate a home directory")?;
    Ok(Consumer::Global {
        agents: home.join(".agents"),
        claude: home.join(".claude"),
    })
}

/// Build a [`Consumer`] for prune/relink from `-g` + an optional path.
/// Existence of a project path is validated by the core op (ADR-0001).
fn make_consumer(global: bool, consumer: Option<String>) -> Result<Consumer> {
    match (global, consumer) {
        (true, Some(_)) => bail!("--global takes no consumer path"),
        (true, None) => global_consumer(),
        (false, Some(path)) => Ok(Consumer::project(path)),
        (false, None) => bail!("need a project path (or use --global)"),
    }
}

/// Split a link/unlink positional list into (consumer, skills). With `-g` every
/// arg is a skill; otherwise the first arg is the project path.
fn split_consumer_args(global: bool, mut args: Vec<String>) -> Result<(Consumer, Vec<String>)> {
    if global {
        Ok((global_consumer()?, args))
    } else {
        if args.len() < 2 {
            bail!("need a project path and at least one skill (or use --global)");
        }
        let consumer = Consumer::project(args.remove(0));
        Ok((consumer, args))
    }
}

fn run_link(sd: &Skilldock, consumer: Consumer, skills: &[String], force: bool) -> Result<()> {
    let out = core::link(sd, &consumer, skills, force)?;
    for name in &out.linked {
        println!("linked {name}");
    }
    for name in &out.already {
        println!("already linked {name}");
    }
    Ok(())
}

fn run_unlink(sd: &Skilldock, consumer: Consumer, skills: &[String]) -> Result<()> {
    let out = core::unlink(sd, &consumer, skills)?;
    for name in &out.removed {
        println!("unlinked {name}");
    }
    for name in &out.missing {
        println!("not linked {name}");
    }
    if out.deregistered {
        println!("deregistered (no links left)");
    }
    Ok(())
}

fn run_prune(sd: &Skilldock, consumer: Consumer) -> Result<()> {
    let out = core::prune(sd, &consumer)?;
    if out.pruned.is_empty() {
        println!("no dangling links");
    }
    for name in &out.pruned {
        println!("pruned {name}");
    }
    if out.deregistered {
        println!("deregistered (no links left)");
    }
    Ok(())
}

fn run_relink(sd: &Skilldock, consumer: Consumer) -> Result<()> {
    let out = core::relink(sd, &consumer)?;
    for name in &out.repointed {
        println!("repointed {name}");
    }
    println!(
        "{} repointed, {} already current",
        out.repointed.len(),
        out.unchanged.len()
    );
    Ok(())
}

fn run_register(sd: &Skilldock, consumers: &[String], remove: bool) -> Result<()> {
    for c in consumers {
        let path = Path::new(c);
        if remove {
            let done = core::deregister(sd, path)?;
            println!(
                "{} {c}",
                if done {
                    "deregistered"
                } else {
                    "not registered"
                }
            );
        } else {
            let done = core::register(sd, path)?;
            println!(
                "{} {c}",
                if done {
                    "registered"
                } else {
                    "already registered"
                }
            );
        }
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
        short_sha(&outcome.resolved),
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

fn run_remove(sd: &Skilldock, targets: &[String]) -> Result<()> {
    for target in targets {
        let out = core::remove(sd, target)?;
        for name in &out.removed {
            println!("removed {name}");
        }
        for repo in &out.pruned_clones {
            println!("pruned Cache clone {repo}");
        }
    }
    Ok(())
}

fn run_update(sd: &Skilldock, repos: &[String]) -> Result<()> {
    let out = core::update(sd, repos)?;
    if out.repos.is_empty() {
        println!("nothing to update");
    }
    for r in &out.repos {
        if r.moved {
            let from = r.from.as_deref().map(short_sha).unwrap_or("(new)");
            println!("updated {}  {} -> {}", r.repo, from, short_sha(&r.to));
        } else {
            println!("up to date {}", r.repo);
        }
    }
    Ok(())
}

/// The first 12 chars of a commit SHA, for display.
fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
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
