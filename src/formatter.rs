use crate::cli::{FormatOptions, OutputPreset};
use crate::model::{CommitMeta, DiffDocument, DiffLine, FileDiff, FileStatus, LineKind};
use std::borrow::Cow;
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

pub fn render_document(doc: &DiffDocument, opts: &FormatOptions, cfg: &RenderConfig) -> String {
    let mut out = String::new();
    let mut first_block = true;
    let is_ai = opts.preset == OutputPreset::Ai;

    for commit in &doc.commits {
        if let Some(meta) = &commit.meta {
            if let Some(header) = format_commit_meta(meta, opts, cfg) {
                if !first_block && !is_ai {
                    out.push('\n');
                }
                first_block = false;
                out.push_str(&header);
                out.push('\n');
            }
        }

        for file in &commit.files {
            if !first_block && !is_ai {
                out.push('\n');
            }
            first_block = false;
            render_file(&mut out, file, opts, cfg);
        }
    }

    if !doc.skipped_files.is_empty() {
        if !first_block && !is_ai {
            out.push('\n');
        }
        let msg = format_skipped(&doc.skipped_files, opts.preset);
        if cfg.use_color {
            let _ = writeln!(out, "{DIM}{msg}{RESET}");
        } else {
            let _ = writeln!(out, "{msg}");
        }
    }

    out
}

fn format_skipped(skipped: &[String], preset: OutputPreset) -> String {
    match preset {
        OutputPreset::Human => {
            let count = skipped.len();
            let noun = if count == 1 { "file" } else { "files" };
            format!(
                "Skipped {count} generated {noun}: {} (use --all-files to include)",
                skipped.join(", ")
            )
        }
        OutputPreset::Default => format!("skipped: {}", skipped.join(", ")),
        OutputPreset::Ai => format!("skip:{}", skipped.join(",")),
    }
}

fn format_commit_meta(
    meta: &CommitMeta,
    opts: &FormatOptions,
    cfg: &RenderConfig,
) -> Option<String> {
    let mut parts = Vec::new();
    if opts.show_hash && !meta.hash.is_empty() {
        parts.push(meta.hash.clone());
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

    let sep = if opts.preset == OutputPreset::Ai {
        "|"
    } else {
        " | "
    };
    let joined = parts.join(sep);

    if cfg.markdown {
        Some(format!("### {joined}"))
    } else if cfg.use_color {
        Some(format!("{BOLD}{YELLOW}{joined}{RESET}"))
    } else {
        Some(joined)
    }
}

fn render_file(out: &mut String, file: &FileDiff, opts: &FormatOptions, cfg: &RenderConfig) {
    let is_human = opts.preset == OutputPreset::Human;
    let is_ai = opts.preset == OutputPreset::Ai;

    let title = if is_human {
        format!("{}   [{}]", file.display_path(), file.status.badge())
    } else if is_ai {
        format!("{}[{}]", file.display_path(), file.status.badge())
    } else {
        format!("{} [{}]", file.display_path(), file.status.badge())
    };

    if cfg.markdown {
        let _ = writeln!(out, "## {title}");
        if file.hunks.is_empty() && !opts.show_headers {
            return;
        }
        let _ = writeln!(out, "```diff");
    } else if is_human {
        if cfg.use_color {
            let _ = writeln!(out, "{BOLD}{CYAN}=== {title} ==={RESET}");
        } else {
            let _ = writeln!(out, "=== {title} ===");
        }
    } else if cfg.use_color {
        let _ = writeln!(out, "{BOLD}{CYAN}{title}{RESET}");
    } else {
        let _ = writeln!(out, "{title}");
    }

    if opts.show_headers {
        for h in &file.raw_headers {
            if cfg.use_color {
                let _ = writeln!(out, "{DIM}{h}{RESET}");
            } else {
                let _ = writeln!(out, "{h}");
            }
        }
    }

    let num_width = if opts.show_line_numbers {
        file.max_line_number().max(1).to_string().len()
    } else {
        0
    };

    let omit_plus_prefix = is_ai && file.status == FileStatus::Added;

    for (idx, hunk) in file.hunks.iter().enumerate() {
        if opts.show_hunks {
            if cfg.use_color {
                let _ = writeln!(out, "{CYAN}{}{RESET}", hunk.header);
            } else {
                let _ = writeln!(out, "{}", hunk.header);
            }
        } else if is_human {
            if idx > 0 {
                if cfg.use_color {
                    let _ = writeln!(out, "  {DIM}---{RESET}");
                } else {
                    let _ = writeln!(out, "  ---");
                }
            }
        } else if file.status != FileStatus::Added {
            let anchor = if hunk.old_start == hunk.new_start {
                format!("@{}", hunk.new_start)
            } else {
                format!("@{},{}", hunk.old_start, hunk.new_start)
            };
            if cfg.use_color {
                let _ = writeln!(out, "{CYAN}{anchor}{RESET}");
            } else {
                let _ = writeln!(out, "{anchor}");
            }
        }

        for line in &hunk.lines {
            render_line(out, line, opts, cfg, num_width, omit_plus_prefix);
        }
    }

    if cfg.markdown {
        let _ = writeln!(out, "```");
    }
}

fn render_line(
    out: &mut String,
    line: &DiffLine,
    opts: &FormatOptions,
    cfg: &RenderConfig,
    num_width: usize,
    omit_plus_prefix: bool,
) {
    let is_ai = opts.preset == OutputPreset::Ai;
    let is_human = opts.preset == OutputPreset::Human;

    if line.kind == LineKind::NoNewline {
        if opts.show_no_newline {
            if cfg.use_color {
                let _ = writeln!(out, "{DIM}{}{RESET}", line.content);
            } else {
                let _ = writeln!(out, "{}", line.content);
            }
        }
        return;
    }

    if is_ai && line.content.trim().is_empty() {
        return;
    }

    let content: Cow<'_, str> = if is_ai {
        compress_ai_line(&line.content)
    } else {
        Cow::Borrowed(&line.content)
    };

    let (sigil, color) = match line.kind {
        LineKind::Addition => ('+', GREEN),
        LineKind::Deletion => ('-', RED),
        LineKind::Context | LineKind::NoNewline => (' ', DIM),
    };

    if opts.show_line_numbers {
        let gutter = if is_human {
            format!(
                "  {sigil} {:>width$} | ",
                line.line_number,
                width = num_width
            )
        } else {
            format!("{sigil}{:>width$}|", line.line_number, width = num_width)
        };
        if cfg.use_color {
            let _ = writeln!(out, "{color}{gutter}{content}{RESET}");
        } else {
            let _ = writeln!(out, "{gutter}{content}");
        }
        return;
    }

    if omit_plus_prefix && line.kind == LineKind::Addition {
        if cfg.use_color {
            let _ = writeln!(out, "{color}{content}{RESET}");
        } else {
            let _ = writeln!(out, "{content}");
        }
    } else if cfg.use_color {
        let _ = writeln!(out, "{color}{sigil}{content}{RESET}");
    } else {
        let _ = writeln!(out, "{sigil}{content}");
    }
}

fn compress_ai_line(raw: &str) -> Cow<'_, str> {
    let trimmed_end = raw.trim_end();
    let leading_spaces = trimmed_end.bytes().take_while(|&b| b == b' ').count();

    if leading_spaces < 2 {
        return Cow::Borrowed(trimmed_end);
    }

    let rest = &trimmed_end[leading_spaces..];
    let (tabs, rem) = if leading_spaces % 4 == 0 {
        (leading_spaces / 4, 0)
    } else if leading_spaces % 2 == 0 {
        (leading_spaces / 2, 0)
    } else {
        (leading_spaces / 4, leading_spaces % 4)
    };

    let mut buf = String::with_capacity(tabs + rem + rest.len());
    for _ in 0..tabs {
        buf.push('\t');
    }
    for _ in 0..rem {
        buf.push(' ');
    }
    buf.push_str(rest);
    Cow::Owned(buf)
}
