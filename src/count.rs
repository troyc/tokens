use std::ops::Range;
use std::sync::OnceLock;

use fancy_regex::Regex;

use crate::bpe::{count_piece, piece_token_ranges, ranks};

/// Official `o200k_base` split pattern from openai/tiktoken `openai_public.py`.
const O200K_PATTERN: &str = concat!(
    r"[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
    r"|[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
    r"|\p{N}{1,3}",
    r"| ?[^\s\p{L}\p{N}]+[\r\n/]*",
    r"|\s*[\r\n]+",
    r"|\s+(?!\S)",
    r"|\s+",
);

fn splitter() -> &'static Regex {
    static SPLITTER: OnceLock<Regex> = OnceLock::new();
    SPLITTER.get_or_init(|| Regex::new(O200K_PATTERN).expect("o200k_base split pattern"))
}

/// Count `o200k_base` tokens in `text`. Special tokens are treated as ordinary text.
pub fn count(text: &str) -> usize {
    let ranks = ranks();
    let mut total = 0;
    for found in splitter().find_iter(text) {
        let piece = found.expect("o200k_base split").as_str();
        total += count_piece(piece.as_bytes(), ranks);
    }
    total
}

/// Byte ranges of the `o200k_base` tokens in `text`.
pub(crate) fn token_ranges(text: &str) -> Vec<Range<usize>> {
    let ranks = ranks();
    let mut tokens = Vec::new();
    for found in splitter().find_iter(text) {
        let found = found.expect("o200k_base split");
        let offset = found.start();
        tokens.extend(
            piece_token_ranges(found.as_str().as_bytes(), ranks)
                .into_iter()
                .map(|range| offset + range.start..offset + range.end),
        );
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(count(""), 0);
    }

    #[test]
    fn hello_world_is_two_tokens() {
        assert_eq!(count("hello world"), 2);
        assert_eq!(count("Hello, world!"), 4);
    }

    #[test]
    fn counts_are_stable_for_code_snippets() {
        assert_eq!(count("fn main() {}\n"), 4);
        assert_eq!(count("// comment\nfn foo() { let x = 1; }\n"), 14);
        assert_eq!(count("The quick brown fox jumps over the lazy dog."), 10);
    }

    #[test]
    fn token_ranges_match_count_and_cover_the_input() {
        let text = "Hello, 世界!\nfn main() {}\n";
        let ranges = token_ranges(text);
        assert_eq!(ranges.len(), count(text));
        assert_eq!(ranges.first().unwrap().start, 0);
        assert_eq!(ranges.last().unwrap().end, text.len());
        assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
    }
}
