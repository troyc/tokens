# tok

Count LLM tokens in a directory tree.

```bash
tok [PATH]
tok -a [PATH]
```

`PATH` defaults to `.`. A file is counted on its own; a directory is walked recursively. Hidden files, gitignored paths (including `target/`), and lockfiles (`Cargo.lock`, `package-lock.json`, `*.lock`, …) are skipped unless you name a file directly. `-l` / `--lockfiles` includes lockfiles.

Default output is a `wc`-style list of per-file totals and a last line for the path you passed:

```
   4218  src/lib.rs
    892  src/main.rs
     41  README.md
   5151  .
```

`-a` / `--all` adds a Rust-aware split into code/other, comments, and tests:

```
total   code  comments  tests  path
 4218   3100       400    718  src/lib.rs
  892    700        80    112  src/main.rs
   41     41         0      0  README.md
 5151   3841       480    830  .
```

On `.rs` files, comments are real `//` / `/* */` tokens (not markers inside strings), tests are `#[cfg(test)]` items, `#[test]` functions, and Cargo `tests/` integration files. Other file types put every token in `code`.

## Encoding

Counts use OpenAI `o200k_base` (GPT-4o, GPT-4.1, GPT-5, o-series). The rank table is the official public file; the split regex and BPE merge are implemented in this repo.

The integer is not a billing-grade count for Claude, Grok, Gemini, Qwen, or other labs. Those tokenizers disagree on the exact number and but generally agree on magnitude for code.

## Build

Rust 1.97.0, edition 2024:

```bash
cargo build --release
```
