use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bgd",
    version,
    about = "Token-minimized, customizable Git diff output for humans and AI"
)]
pub struct RawCli {
    /// Write output to a file instead of stdout (.md wraps diffs in fenced blocks)
    #[arg(short = 'o', long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Lines of unchanged context around edits (default: 0)
    #[arg(short = 'c', long = "context", default_value_t = 0, value_name = "N")]
    pub context: usize,

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

    /// Include @@ hunk range headers
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
    pub show_headers: bool,
    pub show_hunks: bool,
    pub show_hash: bool,
    pub show_author: bool,
    pub show_date: bool,
    pub show_message: bool,
}

impl RawCli {
    pub fn into_config(self) -> AppConfig {
        let show_hash = self.hash || self.meta;
        let show_author = self.author || self.meta;
        let show_date = self.date || self.meta;
        let show_message = self.message || self.meta;

        let context = if self.raw && self.context == 0 {
            3
        } else {
            self.context
        };

        let explicit_paths: Vec<String> = self
            .passthrough
            .iter()
            .filter(|arg| !arg.starts_with('-') && std::path::Path::new(arg).exists())
            .cloned()
            .collect();

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
                show_headers: self.headers || self.raw,
                show_hunks: self.hunks || self.raw,
                show_hash,
                show_author,
                show_date,
                show_message,
            },
            color_override,
        }
    }
}
