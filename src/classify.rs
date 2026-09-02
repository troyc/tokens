use std::ops::Range;
use std::path::{Component, Path};

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, ImplItem, Item, Meta, Stmt, Token, TraitItem, punctuated::Punctuated};

use crate::count::count;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Breakdown {
    pub code: usize,
    pub comments: usize,
    pub tests: usize,
}

impl Breakdown {
    pub fn total(self) -> usize {
        self.code + self.comments + self.tests
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Code,
    Comment,
}

/// Split `source` into code / comments / tests and count each bucket.
///
/// Non-Rust files put every token in `code`. Inside a test range, comments
/// count as tests (purpose-based: how much of the file is droppable test code).
pub fn breakdown(path: &Path, source: &str) -> Breakdown {
    if path.extension().is_none_or(|ext| ext != "rs") {
        return Breakdown {
            code: count(source),
            comments: 0,
            tests: 0,
        };
    }

    let test_ranges = if is_integration_test(path) {
        #[allow(clippy::single_range_in_vec_init)]
        {
            vec![0..source.len()]
        }
    } else {
        syn_test_ranges(source)
    };

    let mut code = String::new();
    let mut comments = String::new();
    let mut tests = String::new();
    for (kind, range) in lex(source) {
        let text = &source[range.clone()];
        let in_test = test_ranges
            .iter()
            .any(|tests| ranges_overlap(tests, &range));
        if in_test {
            tests.push_str(text);
        } else if kind == Kind::Comment {
            comments.push_str(text);
        } else {
            code.push_str(text);
        }
    }

    Breakdown {
        code: count(&code),
        comments: count(&comments),
        tests: count(&tests),
    }
}

fn ranges_overlap(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

/// Cargo integration tests live in a `tests/` directory next to `Cargo.toml`.
pub fn is_integration_test(path: &Path) -> bool {
    let mut dir = match path.parent() {
        Some(dir) => dir,
        None => return false,
    };
    loop {
        if dir.join("Cargo.toml").is_file() {
            let Ok(rel) = path.strip_prefix(dir) else {
                return false;
            };
            let mut components = rel.components();
            return matches!(components.next(), Some(Component::Normal(name)) if name == "tests");
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return false,
        }
    }
}

fn syn_test_ranges(source: &str) -> Vec<Range<usize>> {
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    let mut visitor = TestVisitor { ranges: Vec::new() };
    visitor.visit_file(&file);
    visitor.ranges
}

struct TestVisitor {
    ranges: Vec<Range<usize>>,
}

impl<'ast> Visit<'ast> for TestVisitor {
    fn visit_item(&mut self, item: &'ast Item) {
        if let Some(range) = test_item_range(item_attrs(item), item.span()) {
            self.ranges.push(range);
            return;
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if let Some(range) = test_item_range(impl_item_attrs(item), item.span()) {
            self.ranges.push(range);
            return;
        }
        syn::visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if let Some(range) = test_item_range(trait_item_attrs(item), item.span()) {
            self.ranges.push(range);
            return;
        }
        syn::visit::visit_trait_item(self, item);
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let Stmt::Local(local) = stmt
            && let Some(range) = test_item_range(&local.attrs, local.span())
        {
            self.ranges.push(range);
            return;
        }
        syn::visit::visit_stmt(self, stmt);
    }
}

fn test_item_range(attrs: &[Attribute], span: Span) -> Option<Range<usize>> {
    if !attrs.iter().any(is_test_related_attr) {
        return None;
    }
    Some(span_with_attrs(attrs, span))
}

fn span_with_attrs(attrs: &[Attribute], span: Span) -> Range<usize> {
    let mut range = span.byte_range();
    for attr in attrs {
        let attr_range = attr.span().byte_range();
        range.start = range.start.min(attr_range.start);
        range.end = range.end.max(attr_range.end);
    }
    range
}

fn is_test_related_attr(attr: &Attribute) -> bool {
    is_cfg_test(attr) || is_test_fn_attr(attr)
}

fn is_test_fn_attr(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
}

fn is_cfg_test(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let Ok(args) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return false;
    };
    args.iter().any(meta_enables_test)
}

fn meta_enables_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("all") => {
            parse_cfg_list(list).any(|m| meta_enables_test(&m))
        }
        Meta::List(list) if list.path.is_ident("any") => {
            parse_cfg_list(list).any(|m| meta_enables_test(&m))
        }
        Meta::List(list) if list.path.is_ident("not") => false,
        _ => false,
    }
}

fn parse_cfg_list(list: &syn::MetaList) -> impl Iterator<Item = Meta> {
    list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .into_iter()
        .flatten()
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &TraitItem) -> &[Attribute] {
    match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn lex(source: &str) -> Vec<(Kind, Range<usize>)> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        if let Some(end) = comment_end(bytes, i) {
            spans.push((Kind::Comment, start..end));
            i = end;
            continue;
        }
        i = skip_code_atom(bytes, i);
        spans.push((Kind::Code, start..i));
    }
    spans
}

fn comment_end(bytes: &[u8], i: usize) -> Option<usize> {
    if i + 1 >= bytes.len() || bytes[i] != b'/' {
        return None;
    }
    match bytes[i + 1] {
        b'/' => {
            let mut end = i + 2;
            while end < bytes.len() && bytes[end] != b'\n' {
                end += 1;
            }
            Some(end)
        }
        b'*' => {
            let mut end = i + 2;
            let mut depth = 1usize;
            while end + 1 < bytes.len() {
                if bytes[end] == b'/' && bytes[end + 1] == b'*' {
                    depth += 1;
                    end += 2;
                    continue;
                }
                if bytes[end] == b'*' && bytes[end + 1] == b'/' {
                    depth -= 1;
                    end += 2;
                    if depth == 0 {
                        return Some(end);
                    }
                    continue;
                }
                end += 1;
            }
            Some(bytes.len())
        }
        _ => None,
    }
}

fn skip_code_atom(bytes: &[u8], i: usize) -> usize {
    if let Some(end) = skip_prefixed_string(bytes, i) {
        return end;
    }
    match bytes.get(i).copied() {
        Some(b'"') => skip_quoted(bytes, i + 1, b'"'),
        Some(b'\'') => skip_char_or_lifetime(bytes, i),
        Some(_) => skip_utf8_char(bytes, i),
        None => i,
    }
}

fn skip_utf8_char(bytes: &[u8], i: usize) -> usize {
    debug_assert!(i < bytes.len());
    let mut i = i + 1;
    while i < bytes.len() && bytes[i] & 0b1100_0000 == 0b1000_0000 {
        i += 1;
    }
    i
}

fn skip_prefixed_string(bytes: &[u8], i: usize) -> Option<usize> {
    match bytes.get(i).copied() {
        Some(b'b' | b'c') => match bytes.get(i + 1).copied() {
            Some(b'r') => skip_raw_string(bytes, i + 1),
            Some(b'"') => Some(skip_quoted(bytes, i + 2, b'"')),
            Some(b'\'') if bytes[i] == b'b' => Some(skip_char_or_lifetime(bytes, i + 1)),
            _ => None,
        },
        Some(b'r') => skip_raw_string(bytes, i),
        _ => None,
    }
}

fn skip_raw_string(bytes: &[u8], r_at: usize) -> Option<usize> {
    if bytes.get(r_at).copied() != Some(b'r') {
        return None;
    }
    let mut i = r_at + 1;
    let mut hashes = 0usize;
    while bytes.get(i).copied() == Some(b'#') {
        hashes += 1;
        i += 1;
    }
    if bytes.get(i).copied() != Some(b'"') {
        return None;
    }
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let mut n = 0usize;
            while bytes.get(i + 1 + n).copied() == Some(b'#') {
                n += 1;
            }
            if n == hashes {
                return Some(i + 1 + n);
            }
        }
        i += 1;
    }
    Some(bytes.len())
}

fn skip_quoted(bytes: &[u8], mut i: usize, quote: u8) -> usize {
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i = (i + 2).min(bytes.len());
            continue;
        }
        if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn skip_char_or_lifetime(bytes: &[u8], i: usize) -> usize {
    debug_assert_eq!(bytes.get(i).copied(), Some(b'\''));
    if bytes.get(i + 1).copied() == Some(b'\\') {
        return skip_quoted(bytes, i + 1, b'\'');
    }
    if i + 1 < bytes.len() {
        let after = skip_utf8_char(bytes, i + 1);
        if bytes.get(after).copied() == Some(b'\'') {
            return after + 1;
        }
    }
    i + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn kinds_of(source: &str) -> String {
        let mut merged: Vec<(Kind, String)> = Vec::new();
        for (kind, range) in lex(source) {
            if let Some((last_kind, last_text)) = merged.last_mut()
                && *last_kind == kind
            {
                last_text.push_str(&source[range]);
            } else {
                merged.push((kind, source[range].to_string()));
            }
        }
        merged
            .into_iter()
            .map(|(kind, text)| {
                let tag = match kind {
                    Kind::Code => 'K',
                    Kind::Comment => 'C',
                };
                format!("{tag}:{text:?}")
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn line_and_block_comments() {
        let source = "// line\nfn x() {}\n/* block */\n";
        let text = kinds_of(source);
        assert!(text.contains("C:\"// line\""), "{text}");
        assert!(text.contains("C:\"/* block */\""), "{text}");
        assert!(text.contains("fn x() {}"), "{text}");
    }

    #[test]
    fn nested_block_comments() {
        let source = "/* outer /* inner */ still */ code";
        let spans = lex(source);
        assert_eq!(spans[0].0, Kind::Comment);
        assert_eq!(&source[spans[0].1.clone()], "/* outer /* inner */ still */");
    }

    fn assert_lex_char_aligned(source: &str) {
        let spans = lex(source);
        assert!(
            spans
                .iter()
                .all(|(_, range)| source.is_char_boundary(range.start)
                    && source.is_char_boundary(range.end)),
            "{spans:?}"
        );
        if let Some((_, last)) = spans.last() {
            assert_eq!(last.end, source.len());
        }
        let mut prev = 0;
        for (_, range) in &spans {
            assert_eq!(range.start, prev);
            prev = range.end;
        }
    }

    #[test]
    fn multibyte_char_literal_is_one_atom() {
        let source = "'±'";
        assert_lex_char_aligned(source);
        let spans = lex(source);
        assert_eq!(spans.len(), 1);
        assert_eq!(&source[spans[0].1.clone()], "'±'");
    }

    #[test]
    fn multibyte_utf8_in_code_comments_and_strings() {
        let source = "let π = '±'; let s = \"中🦀\"; // ±\nfn x() {}\n";
        assert_lex_char_aligned(source);
        let _ = kinds_of(source);
        let b = breakdown(Path::new("src/lib.rs"), source);
        assert!(b.code > 0);
        assert!(b.comments > 0);
        assert_eq!(b.total(), b.code + b.comments + b.tests);
    }

    #[test]
    fn comment_markers_inside_strings_are_code() {
        let source = r##"let s = "// not a comment"; let r = r#"/* also not */"#;"##;
        assert!(lex(source).iter().all(|(kind, _)| *kind == Kind::Code));
    }

    #[test]
    fn doc_comments_are_comments() {
        let source = "/// docs\n//! inner\nfn x() {}";
        let comments: String = lex(source)
            .into_iter()
            .filter(|(kind, _)| *kind == Kind::Comment)
            .map(|(_, range)| &source[range])
            .collect();
        assert!(comments.contains("/// docs"));
        assert!(comments.contains("//! inner"));
    }

    #[test]
    fn cfg_test_module_counts_as_tests() {
        let source = "fn prod() {}\n#[cfg(test)]\nmod tests {\n    fn inner() {}\n}\n";
        let ranges = syn_test_ranges(source);
        assert_eq!(ranges.len(), 1);
        assert!(source[ranges[0].clone()].contains("mod tests"));
        assert!(source[ranges[0].clone()].contains("fn inner"));
        assert!(!source[ranges[0].clone()].contains("fn prod"));
    }

    #[test]
    fn test_attribute_on_fn() {
        let source =
            "fn prod() {}\n#[test]\nfn it_works() {}\n#[tokio::test]\nasync fn also() {}\n";
        let ranges = syn_test_ranges(source);
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn cfg_not_test_stays_code() {
        let source = "#[cfg(not(test))]\nfn only_prod() {}\n";
        assert!(syn_test_ranges(source).is_empty());
    }

    #[test]
    fn integration_test_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        fs::create_dir_all(dir.path().join("tests")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        let it = dir.path().join("tests/it.rs");
        let lib = dir.path().join("src/lib.rs");
        fs::write(&it, "").unwrap();
        fs::write(&lib, "").unwrap();
        assert!(is_integration_test(&it));
        assert!(!is_integration_test(&lib));
    }

    #[test]
    fn non_rust_goes_entirely_to_code() {
        let b = breakdown(Path::new("README.md"), "# hi\n");
        assert_eq!(b.comments, 0);
        assert_eq!(b.tests, 0);
        assert_eq!(b.code, count("# hi\n"));
        assert_eq!(b.total(), b.code);
    }

    #[test]
    fn rust_breakdown_splits_comments_and_tests() {
        let source = concat!(
            "// header\n",
            "fn prod() {}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    // test comment\n",
            "    #[test]\n",
            "    fn it_works() {}\n",
            "}\n",
        );
        let b = breakdown(Path::new("src/lib.rs"), source);
        assert!(b.comments > 0, "production comment should count");
        assert!(b.tests > 0, "cfg(test) module should count");
        assert!(b.code > 0, "prod fn should count");
        assert_eq!(b.total(), b.code + b.comments + b.tests);
    }
}
