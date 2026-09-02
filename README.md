# tc

Count LLM tokens in a directory tree.

```bash
tc [PATH]
tc -a [PATH]
tc -l [PATH]
```

`PATH` defaults to `.`. A file is counted on its own; a directory is walked recursively. Hidden files, gitignored paths (including `target/`), and lockfiles (`Cargo.lock`, `package-lock.json`, `*.lock`, …) are skipped unless you name a file directly. `-l` / `--lockfiles` includes lockfiles.

Default output is a `wc`-style list of per-file totals and a last line for the path you passed:

```
   4218  src/lib.rs
    892  src/main.rs
     41  README.md
   5151  .
```

`-a` / `--all` adds a Rust-aware split into code, comments, and tests:

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

The integer is not a billing-grade count for Claude, Grok, Gemini, Qwen, or other labs. Those tokenizers disagree on the exact number and agree on magnitude for code.

## Completions

`tc --generate-completion SHELL` prints a script for `bash`, `zsh`, `fish`, `elvish`, or `powershell`. Source it from your shell config, or install it where your shell already looks.

```bash
# bash (needs bash-completion)
tc --generate-completion bash > ~/.local/share/bash-completion/completions/tc

# zsh — put ~/.zfunc on $fpath before compinit
mkdir -p ~/.zfunc
tc --generate-completion zsh > ~/.zfunc/_tc
# ~/.zshrc:  fpath=(~/.zfunc $fpath)

# fish
tc --generate-completion fish > ~/.config/fish/completions/tc.fish
```

### NixOS

Generate the scripts in `postInstall` so they land in the profile. NixOS bash completion is on by default; enable zsh/fish completion in your config if you use those shells.

```nix
nativeBuildInputs = [ installShellFiles ];
postInstall = lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
  installShellCompletion --cmd tc \
    --bash <($out/bin/tc --generate-completion bash) \
    --fish <($out/bin/tc --generate-completion fish) \
    --zsh  <($out/bin/tc --generate-completion zsh)
'';
```

## Build

Rust 1.97.0, edition 2024:

```bash
cargo build --release
```
