use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Files to count under `root`.
///
/// A file argument is returned as-is, even if gitignore would skip it.
/// A directory is walked with gitignore and hidden-file rules.
pub fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    if !root.exists() {
        bail!("path not found: {}", root.display());
    }
    if !root.is_dir() {
        bail!("not a file or directory: {}", root.display());
    }

    let mut files = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .build();
    for entry in walker {
        let entry = entry.with_context(|| format!("walking {}", root.display()))?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        files.push(entry.into_path());
    }
    files.sort();
    Ok(files)
}

/// Read a UTF-8 text file. Binary files (NUL in the first 8 KiB, or invalid UTF-8) yield `None`.
pub fn read_text(path: &Path) -> Result<Option<String>> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let probe_len = bytes.len().min(8192);
    if bytes[..probe_len].contains(&0) {
        return Ok(None);
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(Some(text)),
        Err(_) => Ok(None),
    }
}

/// Path shown in the report: strip a leading `./`.
pub fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix("./").unwrap_or(&text).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn display_path_strips_dot_slash() {
        assert_eq!(display_path(Path::new("./src/lib.rs")), "src/lib.rs");
        assert_eq!(display_path(Path::new("src/lib.rs")), "src/lib.rs");
    }

    #[test]
    fn gitignore_applies_without_a_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "/target/\n").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("keep.rs"), "fn keep() {}\n").unwrap();
        fs::write(dir.path().join("target/skip.rs"), "fn skip() {}\n").unwrap();

        let files = collect_files(dir.path()).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"keep.rs"));
        assert!(!names.contains(&"skip.rs"));
    }

    #[test]
    fn named_file_is_counted_even_if_gitignored() {
        let dir = tempfile::tempdir().unwrap();
        git_init(dir.path());
        fs::write(dir.path().join(".gitignore"), "secret.txt\n").unwrap();
        fs::write(dir.path().join("secret.txt"), "hello").unwrap();
        fs::write(dir.path().join("keep.txt"), "world").unwrap();

        let files = collect_files(dir.path()).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"keep.txt"));
        assert!(!names.contains(&"secret.txt"));

        let named = collect_files(&dir.path().join("secret.txt")).unwrap();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0].file_name().unwrap(), "secret.txt");
    }

    #[test]
    fn skips_nul_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blob.bin");
        fs::write(&path, b"ok\0nope").unwrap();
        assert!(read_text(&path).unwrap().is_none());
    }

    fn git_init(path: &Path) {
        let status = Command::new("git")
            .args(["-c", "init.defaultBranch=main", "init", "-q"])
            .current_dir(path)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
