use crate::cli::FormatOptions;
use crate::model::{CommitMeta, DiffDocument, FileDiff, LineKind};
use std::fmt::Write;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";

pub struct RenderConfig {
    pub use_color: bool,
    pub markdown: bool,
}

pub fn render_document(
    doc: &DiffDocument,
    opts: &FormatOptions,
    cfg: &RenderConfig,
) -> String {
    let mut out = String::new();
    let mut first_block = true;

    for commit in &doc.commits {
        if let Some(meta) = &commit.meta {
            if let Some(header) = format_commit_meta(meta, opts, cfg) {
                if !first_block {
                    out.push('\n');
                }
                first_block = false;
                out.push_str(&header);
                out.push('\n');
            }
        }

        for file in &commit.files {
            if !first_block {
                out.push('\n');
            }
            first_block = false;
            render_file(&mut out, file, opts, cfg);
        }
    }

    if !doc.skipped_files.is_empty() {
        if !first_block {
            out.push('\n');
        }
        let count = doc.skipped_files.len();
        let noun = if count == 1 { "file" } else { "files" };
        let list = doc.skipped_files.join(", ");
        let msg = format!(
            "Skipped {count} generated {noun}: {list} (use --all-files to include)"
        );
        if cfg.use_color {
            let _ = writeln!(out, "{DIM}{msg}{RESET}");
        } else {
            let _ = writeln!(out, "{msg}");
        }
    }

    out
}

fn format_commit_meta(
    meta: &CommitMeta,
    opts: &FormatOptions,
    cfg: &RenderConfig,
) -> Option<String> {
    let mut parts = Vec::new();
    if opts.show_hash && !meta.hash.is_empty() {
        parts.push(format!("commit {}", meta.hash));
    }
    if opts.show_author && !meta.author.is_empty() {
        parts.push(meta.author.clone());
    }
    if opts.show_date && !meta.date.is_empty() {
        parts.push(meta.date.clone());
    }
    if opts.show_message && !meta.message.is_empty() {
        parts.push(meta.message.clone());
    }

    if parts.is_empty() {
        return None;
    }

    let joined = parts.join(" | ");
    if cfg.markdown {
        Some(format!("### {joined}"))
    } else if cfg.use_color {
        Some(format!("{BOLD}{YELLOW}{joined}{RESET}"))
    } else {
        Some(joined)
    }
}

fn render_file(out: &mut String, file: &FileDiff, opts: &FormatOptions, cfg: &RenderConfig) {
    let title = format!("{}   [{}]", file.display_path(), file.status.badge());

    if cfg.markdown {
        let _ = writeln!(out, "## {title}");
        if file.hunks.is_empty() && !opts.show_headers {
            return;
        }
        let _ = writeln!(out, "```diff");
    } else if cfg.use_color {
        let _ = writeln!(out, "{BOLD}{CYAN}=== {title} ==={RESET}");
    } else {
        let _ = writeln!(out, "=== {title} ===");
    }

    if opts.show_headers {
        for h in &file.raw_headers {
            if cfg.use_color {
                let _ = writeln!(out, "  {DIM}{h}{RESET}");
            } else {
                let _ = writeln!(out, "  {h}");
            }
        }
    }

    let num_width = file.max_line_number().max(1).to_string().len();

    for (idx, hunk) in file.hunks.iter().enumerate() {
        if opts.show_hunks {
            if cfg.use_color {
                let _ = writeln!(out, "  {CYAN}{}{RESET}", hunk.header);
            } else {
                let _ = writeln!(out, "  {}", hunk.header);
            }
        } else if idx > 0 {
            if cfg.use_color {
                let _ = writeln!(out, "  {DIM}---{RESET}");
            } else {
                let _ = writeln!(out, "  ---");
            }
        }

        for line in &hunk.lines {
            let (sigil, color) = match line.kind {
                LineKind::Addition => ('+', GREEN),
                LineKind::Deletion => ('-', RED),
                LineKind::Context => (' ', DIM),
            };

            if cfg.use_color {
                let _ = writeln!(
                    out,
                    "  {color}{sigil} {:>width$} | {}{RESET}",
                    line.line_number,
                    line.content,
                    width = num_width
                );
            } else {
                let _ = writeln!(
                    out,
                    "  {sigil} {:>width$} | {}",
                    line.line_number,
                    line.content,
                    width = num_width
                );
            }
        }
    }

    if cfg.markdown {
        let _ = writeln!(out, "```");
    }
}
