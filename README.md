# Better Git Diff CLI

A Rust-based command-line tool to generate clean, token-minimized, and customizable Git diffs for humans and AI workflows.
Supports presets (`--ai`, `--human`), automatic lockfile and binary filtering, opt-in commit metadata, native `git diff` flag passthrough, and direct export to text or Markdown files.

## Token Benchmark

Measured on an identical working-tree diff across all files (`--all-files` included so both commands process the exact same file set):

- **Baseline:** `git diff` (1,172 lines | 38,291 chars)
- **Target:** `better-git-diff --ai --all-files` (782 lines | 21,697 chars)
- **Size Reduction:** -33.3% (-390) lines | -43.3% (-16,594) chars

| Model                                              | `git diff` | `better-git-diff` |         Improvement |
| :------------------------------------------------- | ---------: | ----------------: | ------------------: |
| GPT-6.1 Sol / Astra / Luna (`o200k_base`)          |     10,109 |             7,150 | **-29.3% (-2,959)** |
| GPT-4 / 3.5 (`cl100k_base`)                        |     10,063 |             7,128 | **-29.2% (-2,935)** |
| DeepSeek V4.1 Flash / V4 Pro                       |     10,682 |             7,672 | **-28.2% (-3,010)** |
| Qwen 3 / 3.8                                       |     10,280 |             7,438 | **-27.6% (-2,842)** |
| Mistral Large 4                                    |     10,306 |             7,672 | **-25.6% (-2,634)** |
| Gemini 3.8 Flash / 3.1 Pro _(proxy)_               |     12,586 |             8,748 | **-30.5% (-3,838)** |
| Claude Fable 5.1 / Opus 5.5 / Sonnet 5.5 _(proxy)_ |     10,941 |             6,200 | **-43.3% (-4,741)** |

## Example Output

Default output (`better-git-diff`):

```text
src/parser.rs [M]
@142,145
-let raw = fetch_raw();
+let raw = fetch_clean_patch()?;

src/new_module.rs [A]
+pub fn run() -> bool {
+    true
+}

skipped: Cargo.lock
```

Human-readable preset (`better-git-diff -H`):

```text
=== src/parser.rs   [M] ===
    140 | fn parse() {
    141 |     init();
  - 142 |     let raw = fetch_raw();
  + 145 |     let raw = fetch_clean_patch()?;
    146 |     finish(raw);
    147 | }

Skipped 1 generated file: Cargo.lock (use --all-files to include)
```

AI-optimized preset (`better-git-diff --ai`):

```text
src/parser.rs[M]
@142,145
-	let raw = fetch_raw();
+	let raw = fetch_clean_patch()?;
src/new_module.rs[A]
pub fn run() -> bool {
	true
}
skip:Cargo.lock
```

## Building from Source

Requires Rust (2021 edition) and `git` available on `PATH`:

```bash
git clone [https://github.com/HardBoss07/better-git-diff.git](https://github.com/HardBoss07/better-git-diff.git)
cd better-git-diff
cargo install --path .
```

Or compile a standalone release binary without installing to Cargo's bin directory:

```bash
cargo build --release
```

The compiled executable will be located at `target/release/better-git-diff` (`target\release\better-git-diff` on Windows).

## Usage

```text
better-git-diff [OPTIONS] [PASSTHROUGH]...
```

- `[PASSTHROUGH]...`: Revision ranges (`HEAD~2..HEAD`), file or directory paths (`src/main.rs`), or native `git diff` flags (`--staged`, `-w`, `--stat`). Defaults to unstaged changes across the current Git repository.

## Options

| Option                    | Description                                 | Default / Base Value | Notes                                                                                   |
| ------------------------- | ------------------------------------------- | -------------------- | --------------------------------------------------------------------------------------- |
| `-o, --output [<PATH>]`   | Write output to a file instead of stdout    | `None`               | Defaults to `./diff.txt` when passed without a path; `.md` wraps diffs in fenced blocks |
| `-H, --human`             | Human-readable terminal preset              | `false`              | Enables `-c 2` context, line-number gutters, and visual dividers; conflicts with `--ai` |
| `--ai`                    | AI-optimized token-minimization preset      | `false`              | Strips empty lines, compresses indents to tabs, and omits redundant `+` on added files  |
| `-c, --context <N>`       | Lines of unchanged context around edits     | `0`                  | Defaults to `2` with `--human`, or `3` with `--raw`                                     |
| `-n, --line-numbers`      | Include per-line old/new line numbers       | `false`              | Enabled automatically by `--human`                                                      |
| `-a, --all`               | Include both staged and unstaged changes    | `false`              | Diffs the working tree directly against `HEAD`                                          |
| `-u, --untracked`         | Include untracked working-tree files        | `false`              | By default, untracked files are ignored to match `git diff`                             |
| `--all-files`             | Include machine-generated and binary files  | `false`              | By default, lockfiles, minified assets, sourcemaps, and binaries are skipped            |
| `-e, --exclude <PATTERN>` | Additional file or glob patterns to exclude | `None`               | Repeatable (e.g., `-e "*.tmp" -e "dist/*"`)                                             |
| `--headers`               | Include standard Git file headers           | `false`              | Emits `diff --git`, `index`, `---`, and `+++` lines                                     |
| `--hunks`                 | Include full `@@` hunk range headers        | `false`              | Replaces compact `@line` anchors with full Git hunk headers                             |
| `--hash`                  | Include commit short hash                   | `false`              | Applies when diffing commits or revision ranges                                         |
| `--author`                | Include commit author                       | `false`              | Applies when diffing commits or revision ranges                                         |
| `--date`                  | Include commit ISO date                     | `false`              | Applies when diffing commits or revision ranges                                         |
| `--message`               | Include commit subject message              | `false`              | Applies when diffing commits or revision ranges                                         |
| `--meta`                  | Include all commit metadata                 | `false`              | Shorthand for `--hash --author --date --message`                                        |
| `--raw`                   | Full raw Git diff fidelity                  | `false`              | Enables `--headers --hunks -c 3 --all-files`                                            |
| `--color`                 | Force ANSI color output                     | `false`              | Conflicts with `--no-color`; ignored when writing to `-o`                               |
| `--no-color`              | Disable ANSI color output                   | `false`              | Also disabled automatically when stdout is piped or `NO_COLOR` is set                   |
| `-h, --help`              | Print help message                          | \-                   | \-                                                                                      |
| `-V, --version`           | Print version                               | \-                   | \-                                                                                      |

## Examples

Print a clean, zero-context diff of unstaged changes in the terminal:

```bash
better-git-diff
```

Diff a single file:

```bash
better-git-diff src/parser.rs
```

Use the human-readable preset with surrounding context and line-number gutters:

```bash
better-git-diff -H
```

Use the AI-optimized preset and write directly to `./diff.txt`:

```bash
better-git-diff --ai -o
```

Include both staged and unstaged changes plus untracked files, exported to Markdown:

```bash
better-git-diff -a -u -o review.md
```

Inspect a commit range with author, date, hash, and commit message:

```bash
better-git-diff --meta HEAD~2..HEAD
```

Include 3 lines of context and exclude temporary files:

```bash
better-git-diff -c 3 -e "*.tmp"
```

Reproduce standard `git diff` fidelity including lockfiles and full headers:

```bash
better-git-diff --raw
```

Pass native `git diff` flags through directly:

```bash
better-git-diff --staged -w
better-git-diff --stat
```

## Notes

- Machine-generated files (`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `go.sum`, `*.min.js`, `*.min.css`, `*.map`, and binary files) are excluded by default unless `--all-files` is passed or the file path is explicitly specified on the command line.
- Untracked files are ignored by default to preserve `git diff` parity unless `-u` / `--untracked` is passed.
- ANSI color escape sequences are stripped automatically when piping output, setting `NO_COLOR`, or writing to a file via `-o` so downstream tools and LLMs never waste tokens parsing terminal codes.
- Exporting to a `.md` or `.markdown` target wraps each file's output in a Markdown heading and fenced `diff` block.
- Any revision ranges, file paths, or native `git diff` flags not claimed by `better-git-diff` are forwarded directly to `git --no-pager diff --no-color`.

## License

This project is licensed under the MIT License.
