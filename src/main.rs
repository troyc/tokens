use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::{CommandFactory, Parser, ValueHint};
use clap_complete::{Shell, generate};

use tc::classify::breakdown;
use tc::count::count;
use tc::report::{self, FileCount};
use tc::walk::{self, display_path};

#[derive(Debug, Parser)]
#[command(name = "tc", version, about = "Count LLM tokens in a directory tree")]
struct Cli {
    /// Show a code / comments / tests breakdown (Rust files).
    #[arg(short, long)]
    all: bool,

    /// Include lockfiles such as Cargo.lock.
    #[arg(short, long)]
    lockfiles: bool,

    /// Print a completion script for the given shell.
    #[arg(long, value_enum, value_name = "SHELL")]
    generate_completion: Option<Shell>,

    /// File or directory to count.
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(shell) = cli.generate_completion {
        let mut cmd = Cli::command();
        let name = cmd.get_name().to_string();
        generate(shell, &mut cmd, name, &mut io::stdout());
        return Ok(());
    }
    let files = walk::collect_files(&cli.path, cli.lockfiles)?;
    let mut rows = Vec::new();
    for file in files {
        let Some(text) = walk::read_text(&file)? else {
            continue;
        };
        let path = display_path(&file);
        let row = if cli.all {
            let parts = breakdown(&file, &text);
            FileCount {
                path,
                total: parts.total(),
                code: parts.code,
                comments: parts.comments,
                tests: parts.tests,
            }
        } else {
            let total = count(&text);
            FileCount {
                path,
                total,
                code: total,
                comments: 0,
                tests: 0,
            }
        };
        rows.push(row);
    }
    print!(
        "{}",
        report::render(&rows, cli.all, &display_path(&cli.path))
    );
    Ok(())
}
