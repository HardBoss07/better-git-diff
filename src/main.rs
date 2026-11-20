mod cli;
mod error;
mod filter;
mod formatter;
mod git;
mod model;
mod output;
mod parser;

use clap::Parser;
use cli::RawCli;
use error::AppError;
use filter::FileFilter;
use formatter::{render_document, RenderConfig};
use git::{fetch_diff, GitOutput};
use output::{is_markdown_target, resolve_use_color, write_result};
use parser::parse_diff;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("better-git-diff: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), AppError> {
    let config = RawCli::parse().into_config();
    let git_output = fetch_diff(&config.git)?;

    let rendered = match git_output {
        GitOutput::RawPassthrough(raw) => raw,
        GitOutput::UnifiedDiff {
            patch,
            untracked_paths,
        } => {
            let filter = FileFilter::new(&config.filter);
            let doc = parse_diff(&patch, &untracked_paths, &filter)?;
            if doc.is_empty() {
                String::new()
            } else {
                let render_cfg = RenderConfig {
                    use_color: resolve_use_color(config.output.as_deref(), config.color_override),
                    markdown: is_markdown_target(config.output.as_deref()),
                };
                render_document(&doc, &config.format, &render_cfg)
            }
        }
    };

    write_result(&rendered, config.output.as_deref())
}
