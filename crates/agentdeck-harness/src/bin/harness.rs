//! `agentdeck-harness` CLI.
//!
//! Subcommands:
//!
//! - `record`  — capture a real agent session into a fixture (not yet
//!   implemented; needs the monitor core to drive process scanning).
//! - `redact`  — apply redaction rules to a captured fixture in place.
//! - `replay`  — load a fixture and run it through the monitor (not yet
//!   implemented; waits for adapters).
//! - `verify`  — load a fixture and assert its `expectations` (not yet
//!   implemented; waits for adapters).
//!
//! See `planning/agent-harness-notes.md` for the full design.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};

use agentdeck_harness::{Fixture, Redactor};

#[derive(Debug, Parser)]
#[command(name = "agentdeck-harness", author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Capture a real agent session into a fixture. Not yet implemented.
    Record {
        /// Which agent is being recorded.
        #[arg(long)]
        agent: String,
        /// Output directory under `fixtures/sessions/`.
        #[arg(long)]
        out: PathBuf,
    },

    /// Apply redaction rules to a captured fixture in place.
    Redact {
        /// Path to a fixture descriptor or its containing directory.
        path: PathBuf,
        /// Absolute path of the recorder's home directory.
        #[arg(long)]
        home: Option<PathBuf>,
        /// Absolute path of the captured repository.
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Username to redact in any path or text.
        #[arg(long)]
        username: Option<String>,
    },

    /// Load a fixture and print its summary. Full replay against
    /// adapters is not yet implemented.
    Replay {
        /// Path to the fixture descriptor file.
        fixture: PathBuf,
    },

    /// Load a fixture and assert its expectations. Not yet implemented.
    Verify {
        /// Path to the fixture descriptor file.
        fixture: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Record { agent, out } => {
            let _ = (agent, out);
            anyhow::bail!(
                "`record` is not implemented yet. It needs the monitor core to drive process scanning; see planning/agent-harness-notes.md §5."
            )
        }
        Command::Replay { fixture } => {
            let f =
                Fixture::load(&fixture).with_context(|| format!("loading fixture {fixture:?}"))?;
            println!(
                "Loaded fixture name={:?} agent={} version={} duration_ms={} processes={} files={}",
                f.name,
                f.agent,
                f.agent_version,
                f.duration_ms,
                f.processes.len(),
                f.files.len()
            );
            anyhow::bail!(
                "`replay` is not implemented yet. The fixture loads cleanly; driving it against adapters waits for step 8 of the build plan."
            )
        }
        Command::Verify { fixture } => {
            let _ =
                Fixture::load(&fixture).with_context(|| format!("loading fixture {fixture:?}"))?;
            anyhow::bail!(
                "`verify` is not implemented yet. Expectations checking lands with the replay engine."
            )
        }
        Command::Redact {
            path,
            home,
            repo,
            username,
        } => run_redact(path, home, repo, username),
    }
}

fn run_redact(
    path: PathBuf,
    home: Option<PathBuf>,
    repo: Option<PathBuf>,
    username: Option<String>,
) -> anyhow::Result<()> {
    let mut redactor = Redactor::new();
    if let Some(h) = home {
        redactor = redactor.with_home(h);
    }
    if let Some(r) = repo {
        redactor = redactor.with_repo(r);
    }
    if let Some(u) = username {
        redactor = redactor.with_username(u);
    }

    let count = redact_path(&path, &redactor)?;
    println!("redacted {count} file(s) under {path:?}");
    Ok(())
}

/// Walk `target` (file or directory) and apply text redaction in place
/// to every supported text file.
fn redact_path(target: &Path, redactor: &Redactor) -> anyhow::Result<usize> {
    let mut count = 0;
    if target.is_file() {
        if redact_in_place(target, redactor)? {
            count += 1;
        }
        return Ok(count);
    }
    if !target.is_dir() {
        anyhow::bail!("target does not exist: {target:?}");
    }
    for entry in walkdir::WalkDir::new(target) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if redact_in_place(entry.path(), redactor)? {
            count += 1;
        }
    }
    Ok(count)
}

fn redact_in_place(path: &Path, redactor: &Redactor) -> anyhow::Result<bool> {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return Ok(false);
    };
    let supported = matches!(ext, "md" | "json" | "txt" | "log" | "yaml" | "yml");
    if !supported {
        return Ok(false);
    }
    let original = std::fs::read_to_string(path).with_context(|| format!("reading {path:?}"))?;
    let redacted = redactor.redact(&original);
    if redacted != original {
        std::fs::write(path, redacted).with_context(|| format!("writing {path:?}"))?;
        return Ok(true);
    }
    Ok(false)
}
