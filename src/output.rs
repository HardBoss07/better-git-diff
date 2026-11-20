use crate::error::AppError;
use std::fs::{self, File};
use std::io::{self, BufWriter, IsTerminal, Write};
use std::path::Path;

pub fn resolve_use_color(output_path: Option<&Path>, color_override: Option<bool>) -> bool {
    if output_path.is_some() {
        return false;
    }
    if let Some(forced) = color_override {
        return forced;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    io::stdout().is_terminal()
}

pub fn is_markdown_target(output_path: Option<&Path>) -> bool {
    output_path
        .and_then(|p| p.extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

pub fn write_result(content: &str, output_path: Option<&Path>) -> Result<(), AppError> {
    match output_path {
        Some(path) => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            let file = File::create(path)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(content.as_bytes())?;
            writer.flush()?;
            eprintln!("Wrote diff to {}", path.display());
        }
        None => {
            if content.is_empty() {
                return Ok(());
            }
            let stdout = io::stdout();
            let mut writer = BufWriter::new(stdout.lock());
            writer.write_all(content.as_bytes())?;
            writer.flush()?;
        }
    }
    Ok(())
}
