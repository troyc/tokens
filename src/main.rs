use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use tok::classify::breakdown;
use tok::count::count;
use tok::report::{self, FileCount};
use tok::walk::{self, display_path};

#[derive(Debug, Parser)]
#[command(name = "tok", version, about = "Count LLM tokens in a directory tree")]
struct Cli {
    /// Show a code / comments / tests breakdown (Rust files).
    #[arg(short, long)]
    all: bool,

    /// Include lockfiles such as Cargo.lock.
    #[arg(short, long)]
    lockfiles: bool,

    /// File or directory to count.
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
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
