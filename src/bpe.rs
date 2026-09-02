use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::io::Read;
use std::ops::Range;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::read::GzDecoder;

const RANKS_GZ: &[u8] = include_bytes!("../data/o200k_base.tiktoken.gz");

/// SHA-256 of the official uncompressed `o200k_base.tiktoken` file.
pub const O200K_SHA256: &str = "446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d";

pub fn ranks() -> &'static HashMap<Vec<u8>, u32> {
    static RANKS: OnceLock<HashMap<Vec<u8>, u32>> = OnceLock::new();
    RANKS.get_or_init(|| load_ranks().expect("o200k_base rank table"))
}

pub fn rank_file_bytes() -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(RANKS_GZ);
    let mut bytes = Vec::new();
    decoder
        .read_to_end(&mut bytes)
        .context("decompressing o200k_base ranks")?;
    Ok(bytes)
}

fn load_ranks() -> Result<HashMap<Vec<u8>, u32>> {
    let text = String::from_utf8(rank_file_bytes()?).context("o200k_base ranks are not UTF-8")?;
    let mut ranks = HashMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let Some((token, rank)) = line.split_once(' ') else {
            bail!("rank line {} is missing a space", index + 1);
        };
        let bytes = STANDARD
            .decode(token)
            .with_context(|| format!("rank line {}", index + 1))?;
        let rank: u32 = rank
            .parse()
            .with_context(|| format!("rank line {}", index + 1))?;
        ranks.insert(bytes, rank);
    }
    Ok(ranks)
}

/// Count BPE tokens in one pretokenized piece.
pub fn count_piece(piece: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> usize {
    piece_token_ranges(piece, ranks).len()
}

/// Byte ranges of the BPE tokens in one pretokenized piece.
pub(crate) fn piece_token_ranges(piece: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<Range<usize>> {
    if piece.is_empty() {
        return Vec::new();
    }
    if ranks.contains_key(piece) {
        return std::iter::once(0..piece.len()).collect();
    }

    let mut nodes: Vec<_> = (0..piece.len())
        .map(|start| Node {
            start,
            end: start + 1,
            previous: start.checked_sub(1),
            next: (start + 1 < piece.len()).then_some(start + 1),
            version: 0,
            alive: true,
        })
        .collect();
    let mut candidates = BinaryHeap::new();
    for left in 0..piece.len() - 1 {
        push_candidate(&mut candidates, &nodes, piece, ranks, left);
    }

    while let Some(Reverse(candidate)) = candidates.pop() {
        let left = candidate.left;
        let right = candidate.right;
        if !nodes[left].alive
            || !nodes[right].alive
            || nodes[left].next != Some(right)
            || nodes[left].version != candidate.left_version
            || nodes[right].version != candidate.right_version
        {
            continue;
        }

        let previous = nodes[left].previous;
        let next = nodes[right].next;
        nodes[left].end = nodes[right].end;
        nodes[left].next = next;
        nodes[left].version += 1;
        nodes[right].alive = false;
        if let Some(next) = next {
            nodes[next].previous = Some(left);
        }

        if let Some(previous) = previous {
            push_candidate(&mut candidates, &nodes, piece, ranks, previous);
        }
        push_candidate(&mut candidates, &nodes, piece, ranks, left);
    }

    let mut tokens = Vec::new();
    let mut current = Some(0);
    while let Some(index) = current {
        let node = &nodes[index];
        tokens.push(node.start..node.end);
        current = node.next;
    }
    tokens
}

#[derive(Debug)]
struct Node {
    start: usize,
    end: usize,
    previous: Option<usize>,
    next: Option<usize>,
    version: usize,
    alive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    rank: u32,
    start: usize,
    left: usize,
    right: usize,
    left_version: usize,
    right_version: usize,
}

fn push_candidate(
    candidates: &mut BinaryHeap<Reverse<Candidate>>,
    nodes: &[Node],
    piece: &[u8],
    ranks: &HashMap<Vec<u8>, u32>,
    left: usize,
) {
    let Some(right) = nodes[left].next else {
        return;
    };
    let Some(&rank) = ranks.get(&piece[nodes[left].start..nodes[right].end]) else {
        return;
    };
    candidates.push(Reverse(Candidate {
        rank,
        start: nodes[left].start,
        left,
        right,
        left_version: nodes[left].version,
        right_version: nodes[right].version,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_rank_file_hash_matches() {
        use sha2::{Digest, Sha256};
        let bytes = rank_file_bytes().unwrap();
        let digest = Sha256::digest(&bytes);
        assert_eq!(format!("{digest:x}"), O200K_SHA256);
    }

    #[test]
    fn whole_piece_in_vocab_is_one_token() {
        let ranks = self::ranks();
        assert_eq!(count_piece(b"hello", ranks), 1);
    }

    #[test]
    fn empty_piece_is_zero() {
        assert_eq!(count_piece(b"", ranks()), 0);
    }

    #[test]
    fn priority_queue_matches_reference_merger() {
        let ranks = ranks();
        for piece in [
            vec![b' '; 1_000],
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec(),
            b"the quick brown fox jumps over the lazy dog".to_vec(),
            "中文とemoji 🦀🦀🦀".as_bytes().to_vec(),
        ] {
            assert_eq!(
                piece_token_ranges(&piece, ranks),
                reference_piece_token_ranges(&piece, ranks),
                "piece: {piece:?}"
            );
        }

        const ALPHABET: &[u8] = b" abcdefghijklmnopqrstuvwxyz0123456789\n/*_-";
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for len in 0..128 {
            for _ in 0..16 {
                let mut piece = Vec::with_capacity(len);
                for _ in 0..len {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1);
                    piece.push(ALPHABET[(state as usize) % ALPHABET.len()]);
                }
                assert_eq!(
                    piece_token_ranges(&piece, ranks),
                    reference_piece_token_ranges(&piece, ranks),
                    "piece: {piece:?}"
                );
            }
        }
    }

    #[test]
    fn handles_long_repetitive_pieces() {
        let piece = vec![b' '; 100_000];
        let tokens = piece_token_ranges(&piece, ranks());
        assert_eq!(tokens.first().unwrap().start, 0);
        assert_eq!(tokens.last().unwrap().end, piece.len());
        assert!(tokens.windows(2).all(|pair| pair[0].end == pair[1].start));
    }

    fn reference_piece_token_ranges(
        piece: &[u8],
        ranks: &HashMap<Vec<u8>, u32>,
    ) -> Vec<Range<usize>> {
        if piece.is_empty() {
            return Vec::new();
        }
        if ranks.contains_key(piece) {
            return std::iter::once(0..piece.len()).collect();
        }

        let mut starts: Vec<usize> = (0..piece.len()).collect();
        starts.push(piece.len());
        loop {
            let mut best: Option<(u32, usize)> = None;
            for i in 0..starts.len().saturating_sub(2) {
                let pair = &piece[starts[i]..starts[i + 2]];
                if let Some(&rank) = ranks.get(pair)
                    && best.is_none_or(|(best_rank, _)| rank < best_rank)
                {
                    best = Some((rank, i));
                }
            }
            let Some((_, i)) = best else {
                break;
            };
            starts.remove(i + 1);
        }
        starts
            .windows(2)
            .map(|bounds| bounds[0]..bounds[1])
            .collect()
    }
}
