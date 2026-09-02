use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCount {
    pub path: String,
    pub total: usize,
    pub code: usize,
    pub comments: usize,
    pub tests: usize,
}

pub fn render(rows: &[FileCount], all: bool, total_label: &str) -> String {
    let grand = FileCount {
        path: total_label.to_string(),
        total: rows.iter().map(|row| row.total).sum(),
        code: rows.iter().map(|row| row.code).sum(),
        comments: rows.iter().map(|row| row.comments).sum(),
        tests: rows.iter().map(|row| row.tests).sum(),
    };

    let mut out = String::new();
    if all {
        let total_w = num_width("total", rows.iter().map(|r| r.total).chain([grand.total]));
        let code_w = num_width("code", rows.iter().map(|r| r.code).chain([grand.code]));
        let comments_w = num_width(
            "comments",
            rows.iter().map(|r| r.comments).chain([grand.comments]),
        );
        let tests_w = num_width("tests", rows.iter().map(|r| r.tests).chain([grand.tests]));
        let _ = writeln!(
            out,
            "{:>total_w$}  {:>code_w$}  {:>comments_w$}  {:>tests_w$}  path",
            "total", "code", "comments", "tests"
        );
        for row in rows {
            let _ = writeln!(
                out,
                "{:>total_w$}  {:>code_w$}  {:>comments_w$}  {:>tests_w$}  {}",
                row.total, row.code, row.comments, row.tests, row.path
            );
        }
        let _ = writeln!(
            out,
            "{:>total_w$}  {:>code_w$}  {:>comments_w$}  {:>tests_w$}  {}",
            grand.total, grand.code, grand.comments, grand.tests, grand.path
        );
    } else {
        let width = rows
            .iter()
            .map(|row| row.total)
            .chain([grand.total])
            .map(digit_count)
            .max()
            .unwrap_or(1);
        for row in rows {
            let _ = writeln!(out, "{:>width$}  {}", row.total, row.path);
        }
        let _ = writeln!(out, "{:>width$}  {}", grand.total, grand.path);
    }
    out
}

fn num_width(header: &str, values: impl Iterator<Item = usize>) -> usize {
    values
        .map(digit_count)
        .chain([header.len()])
        .max()
        .unwrap_or(header.len())
}

fn digit_count(n: usize) -> usize {
    n.to_string().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, total: usize, code: usize, comments: usize, tests: usize) -> FileCount {
        FileCount {
            path: path.into(),
            total,
            code,
            comments,
            tests,
        }
    }

    #[test]
    fn default_mode_has_no_header() {
        let text = render(
            &[row("a.rs", 10, 10, 0, 0), row("b.rs", 3, 3, 0, 0)],
            false,
            ".",
        );
        assert!(!text.to_lowercase().contains("total"));
        assert!(text.contains("10  a.rs"));
        assert!(text.contains(" 3  b.rs"));
        assert!(text.contains("13  ."));
        assert_eq!(text.lines().count(), 3);
    }

    #[test]
    fn all_mode_header_and_columns_add_up() {
        let rows = [row("src/lib.rs", 15, 10, 2, 3)];
        assert_eq!(
            rows[0].total,
            rows[0].code + rows[0].comments + rows[0].tests
        );
        let text = render(&rows, true, "src");
        let header = text.lines().next().unwrap();
        assert!(header.contains("total"));
        assert!(header.contains("code"));
        assert!(header.contains("comments"));
        assert!(header.contains("tests"));
        assert!(header.contains("path"));
        assert!(text.contains("src/lib.rs"));
        assert!(text.lines().last().unwrap().contains("src"));
    }
}
