use crate::cli::GitOptions;
use crate::error::AppError;
use std::process::Command;

const PASSTHROUGH_SUMMARY_FLAGS: &[&str] = &[
    "--stat",
    "--numstat",
    "--shortstat",
    "--dirstat",
    "--summary",
    "--name-only",
    "--name-status",
    "--check",
];

pub enum GitOutput {
    UnifiedDiff(String),
    RawPassthrough(String),
}

pub fn fetch_diff(opts: &GitOptions) -> Result<GitOutput, AppError> {
    if is_summary_passthrough(&opts.passthrough) {
        let raw = run_git_command(&build_diff_args(opts, false))?;
        return Ok(GitOutput::RawPassthrough(raw));
    }

    let use_log_mode = opts.wants_commit_meta && has_revision_arg(&opts.passthrough);

    let mut combined = if use_log_mode {
        run_git_command(&build_log_args(opts))?
    } else {
        run_git_command(&build_diff_args(opts, true))?
    };

    if opts.untracked && !use_log_mode {
        let untracked_diff = fetch_untracked_diffs(opts.context)?;
        if !untracked_diff.is_empty() {
            if !combined.is_empty() && !combined.ends_with('\n') {
                combined.push('\n');
            }
            combined.push_str(&untracked_diff);
        }
    }

    Ok(GitOutput::UnifiedDiff(combined))
}

fn build_diff_args(opts: &GitOptions, enforce_clean_patch: bool) -> Vec<String> {
    let mut args = vec![
        "--no-pager".to_string(),
        "-c".to_string(),
        "core.quotepath=false".to_string(),
        "diff".to_string(),
        "--no-color".to_string(),
    ];

    if enforce_clean_patch {
        args.push("--no-ext-diff".to_string());
        args.push(format!("-U{}", opts.context));
    }

    if opts.all_uncommitted && !has_revision_arg(&opts.passthrough) {
        args.push("HEAD".to_string());
    }

    args.extend(opts.passthrough.iter().cloned());
    args
}

fn build_log_args(opts: &GitOptions) -> Vec<String> {
    let mut args = vec![
        "--no-pager".to_string(),
        "-c".to_string(),
        "core.quotepath=false".to_string(),
        "log".to_string(),
        "-p".to_string(),
        "--no-color".to_string(),
        "--no-ext-diff".to_string(),
        format!("-U{}", opts.context),
        "--date=iso-strict".to_string(),
        "--format=BGD_META:%h%x00%an%x00%ad%x00%s".to_string(),
    ];
    args.extend(opts.passthrough.iter().cloned());
    args
}

fn fetch_untracked_diffs(context: usize) -> Result<String, AppError> {
    let list_output = run_git_command(&[
        "--no-pager".to_string(),
        "-c".to_string(),
        "core.quotepath=false".to_string(),
        "ls-files".to_string(),
        "--others".to_string(),
        "--exclude-standard".to_string(),
    ])?;

    let mut result = String::new();
    for path in list_output.lines().filter(|l| !l.trim().is_empty()) {
        let output = Command::new("git")
            .args([
                "--no-pager",
                "-c",
                "core.quotepath=false",
                "diff",
                "--no-color",
                "--no-index",
                &format!("-U{context}"),
                "--",
                "/dev/null",
                path,
            ])
            .output()?;

        // git diff --no-index exits with 1 when differences exist
        if output.status.code() == Some(0) || output.status.code() == Some(1) {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
    }

    Ok(result)
}

fn run_git_command(args: &[String]) -> Result<String, AppError> {
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AppError::GitExecution(if stderr.is_empty() {
            format!("git exited with status {}", output.status)
        } else {
            stderr
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn is_summary_passthrough(args: &[String]) -> bool {
    args.iter()
        .any(|arg| PASSTHROUGH_SUMMARY_FLAGS.iter().any(|flag| arg == flag || arg.starts_with("--stat=")))
}

fn has_revision_arg(args: &[String]) -> bool {
    args.iter().any(|arg| {
        if arg.starts_with('-') || arg == "--" {
            return false;
        }
        !std::path::Path::new(arg).exists()
    })
}
