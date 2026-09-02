use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn tc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tc"))
}

fn git_init(path: &Path) {
    let status = Command::new("git")
        .args(["-c", "init.defaultBranch=main", "init", "-q"])
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    git_init(dir.path());
    fs::write(dir.path().join(".gitignore"), "/target/\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("tests")).unwrap();
    fs::create_dir_all(dir.path().join("target")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        concat!(
            "// production comment\n",
            "pub fn add(a: i32, b: i32) -> i32 {\n",
            "    a + b\n",
            "}\n",
            "\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "    #[test]\n",
            "    fn adds() {\n",
            "        assert_eq!(add(1, 2), 3);\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        dir.path().join("tests/it.rs"),
        "#[test]\nfn integration() {\n    assert!(true);\n}\n",
    )
    .unwrap();
    fs::write(dir.path().join("README.md"), "# demo\n\nA tiny crate.\n").unwrap();
    fs::write(
        dir.path().join("Cargo.lock"),
        "# this file is automatically @generated\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("target/foo.rs"),
        "// ignored build output\nfn ignored() {}\n",
    )
    .unwrap();
    dir
}

fn stdout(cmd: &mut Command) -> String {
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn default_lists_files_and_omits_gitignored_target() {
    let dir = fixture();
    let text = stdout(tc().arg(dir.path()));
    assert!(text.contains("src/lib.rs"), "{text}");
    assert!(text.contains("tests/it.rs"), "{text}");
    assert!(text.contains("README.md"), "{text}");
    assert!(!text.contains("Cargo.lock"), "{text}");
    assert!(!text.contains("target/foo.rs"), "{text}");
    assert!(!text.contains("total"), "{text}");
    let lines: Vec<_> = text.lines().collect();
    assert!(lines.len() >= 4, "{text}");
    let last = lines.last().unwrap();
    assert!(
        last.contains(&dir.path().display().to_string()),
        "total line should name the root:\n{text}"
    );
}

#[test]
fn all_mode_shows_comments_and_tests() {
    let dir = fixture();
    let text = stdout(tc().arg("-a").arg(dir.path()));
    let header = text.lines().next().unwrap();
    assert!(header.contains("total"));
    assert!(header.contains("code"));
    assert!(header.contains("comments"));
    assert!(header.contains("tests"));

    let lib = text
        .lines()
        .find(|line| line.contains("src/lib.rs"))
        .unwrap_or_else(|| panic!("missing lib.rs in:\n{text}"));
    let cols: Vec<_> = lib.split_whitespace().collect();
    // total code comments tests path...
    let comments: usize = cols[2].parse().unwrap();
    let tests: usize = cols[3].parse().unwrap();
    let code: usize = cols[1].parse().unwrap();
    let total: usize = cols[0].parse().unwrap();
    assert!(comments > 0, "{lib}");
    assert!(tests > 0, "{lib}");
    assert!(code > 0, "{lib}");
    assert_eq!(total, code + comments + tests, "{lib}");

    let it = text
        .lines()
        .find(|line| line.contains("tests/it.rs"))
        .unwrap_or_else(|| panic!("missing it.rs in:\n{text}"));
    let cols: Vec<_> = it.split_whitespace().collect();
    let tests: usize = cols[3].parse().unwrap();
    assert!(tests > 0, "{it}");
}

#[test]
fn lockfiles_flag_includes_cargo_lock() {
    let dir = fixture();
    let text = stdout(tc().arg("-l").arg(dir.path()));
    assert!(text.contains("Cargo.lock"), "{text}");
    assert!(text.contains("src/lib.rs"), "{text}");
}

#[test]
fn named_gitignored_file_is_still_counted() {
    let dir = fixture();
    let ignored = dir.path().join("target/foo.rs");
    let text = stdout(tc().arg(&ignored));
    assert!(text.contains("foo.rs"), "{text}");
    assert!(!text.contains("src/lib.rs"), "{text}");
}
