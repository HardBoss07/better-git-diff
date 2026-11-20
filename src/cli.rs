use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "better-git-diff",
    version,
    about = "Token-minimized, customizable Git diff output for humans and AI"
)]
pub struct RawCli {
    /// Write output to a file instead of stdout (defaults to ./diff.txt; .md wraps diffs in fenced blocks)
    #[arg(
        short = 'o',
        long = "output",
        num_args = 0..=1,
        default_missing_value = "diff.txt",
        value_name = "PATH"
    )]
    pub output: Option<PathBuf>,

    /// Human-readable preset (-c 2 context, line-number gutters, visual file/hunk dividers)
    #[arg(short = 'H', long = "human", conflicts_with = "ai")]
    pub human: bool,

    /// AI-optimized preset (lossless token minimization: strips empty lines, compresses indents, omits redundant + on new files)
    #[arg(long = "ai", conflicts_with = "human")]
    pub ai: bool,

    /// Lines of unchanged context around edits (default: 0, or 2 with --human)
    #[arg(short = 'c', long = "context", value_name = "N")]
    pub context: Option<usize>,

    /// Include per-line old/new line numbers
    #[arg(short = 'n', long = "line-numbers")]
    pub line_numbers: bool,

    /// Include both staged and unstaged changes (diffs against HEAD)
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Include untracked working-tree files
    #[arg(short = 'u', long = "untracked")]
    pub untracked: bool,

    /// Include machine-generated files (lockfiles, minified bundles, sourcemaps, binaries)
    #[arg(long = "all-files")]
    pub all_files: bool,

    /// Additional file or glob patterns to exclude (repeatable)
    #[arg(short = 'e', long = "exclude", value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Include standard Git file headers (diff --git, index, ---, +++)
    #[arg(long = "headers")]
    pub headers: bool,

    /// Include full @@ hunk range headers
    #[arg(long = "hunks")]
    pub hunks: bool,

    /// Include commit short hash (when diffing commits/ranges)
    #[arg(long = "hash")]
    pub hash: bool,

    /// Include commit author (when diffing commits/ranges)
    #[arg(long = "author")]
    pub author: bool,

    /// Include commit date (when diffing commits/ranges)
    #[arg(long = "date")]
    pub date: bool,

    /// Include commit subject message (when diffing commits/ranges)
    #[arg(long = "message")]
    pub message: bool,

    /// Shorthand for --hash --author --date --message
    #[arg(long = "meta")]
    pub meta: bool,

    /// Full raw Git diff fidelity (--headers --hunks -c 3 --all-files)
    #[arg(long = "raw")]
    pub raw: bool,

    /// Force ANSI color output
    #[arg(long = "color", conflicts_with = "no_color")]
    pub color: bool,

    /// Disable ANSI color output
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Revision ranges, file paths, or native git diff flags (e.g. --staged, -w, --stat)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub passthrough: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputPreset {
    Default,
    Human,
    Ai,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub output: Option<PathBuf>,
    pub git: GitOptions,
    pub filter: FilterOptions,
    pub format: FormatOptions,
    pub color_override: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct GitOptions {
    pub context: usize,
    pub all_uncommitted: bool,
    pub untracked: bool,
    pub wants_commit_meta: bool,
    pub passthrough: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FilterOptions {
    pub include_generated: bool,
    pub custom_excludes: Vec<String>,
    pub explicit_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub preset: OutputPreset,
    pub show_line_numbers: bool,
    pub show_headers: bool,
    pub show_hunks: bool,
    pub show_hash: bool,
    pub show_author: bool,
    pub show_date: bool,
    pub show_message: bool,
    pub show_no_newline: bool,
}

impl RawCli {
    pub fn into_config(self) -> AppConfig {
        let preset = if self.human {
            OutputPreset::Human
        } else if self.ai {
            OutputPreset::Ai
        } else {
            OutputPreset::Default
        };

        let show_hash = self.hash || self.meta;
        let show_author = self.author || self.meta;
        let show_date = self.date || self.meta;
        let show_message = self.message || self.meta;

        let default_context = match preset {
            OutputPreset::Human => 2,
            OutputPreset::Default | OutputPreset::Ai => 0,
        };

        let context = match self.context {
            Some(c) => c,
            None if self.raw => 3,
            None => default_context,
        };

        let show_line_numbers = self.line_numbers || preset == OutputPreset::Human;
        let explicit_paths = extract_path_candidates(&self.passthrough);

        let color_override = if self.color {
            Some(true)
        } else if self.no_color {
            Some(false)
        } else {
            None
        };

        AppConfig {
            output: self.output,
            git: GitOptions {
                context,
                all_uncommitted: self.all,
                untracked: self.untracked,
                wants_commit_meta: show_hash || show_author || show_date || show_message,
                passthrough: self.passthrough,
            },
            filter: FilterOptions {
                include_generated: self.all_files || self.raw,
                custom_excludes: self.exclude,
                explicit_paths,
            },
            format: FormatOptions {
                preset,
                show_line_numbers,
                show_headers: self.headers || self.raw,
                show_hunks: self.hunks || self.raw,
                show_hash,
                show_author,
                show_date,
                show_message,
                show_no_newline: self.raw,
            },
            color_override,
        }
    }
}

fn extract_path_candidates(args: &[String]) -> Vec<String> {
    let mut after_separator = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg == "--" {
            after_separator = true;
            continue;
        }
        if after_separator || (!arg.starts_with('-') && !arg.contains("..")) {
            paths.push(arg.clone());
        }
    }

    paths
}
