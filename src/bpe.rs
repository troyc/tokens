use std::collections::HashMap;
use std::io::Read;
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
    if piece.is_empty() {
        return 0;
    }
    if ranks.contains_key(piece) {
        return 1;
    }

    // `starts[i]..starts[i+1]` is the i-th symbol. The last entry is the piece length.
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

    starts.len() - 1
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
}
