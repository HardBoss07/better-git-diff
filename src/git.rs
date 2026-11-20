use crate::cli::GitOptions;
use crate::error::AppError;
use std::path::Path;
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
    UnifiedDiff {
        patch: String,
        untracked_paths: Vec<String>,
    },
    RawPassthrough(String),
}

#[derive(Debug, Default)]
struct RevisionInfo {
    has_revision: bool,
    is_range: bool,
    has_limit_flag: bool,
}

pub fn fetch_diff(opts: &GitOptions) -> Result<GitOutput, AppError> {
    if is_summary_passthrough(&opts.passthrough) {
        let rev = inspect_revisions(&opts.passthrough);
        let raw = run_git_command(&build_diff_args(opts, &rev, false))?;
        return Ok(GitOutput::RawPassthrough(raw));
    }

    let rev = if opts.all_uncommitted || opts.wants_commit_meta {
        inspect_revisions(&opts.passthrough)
    } else {
        RevisionInfo::default()
    };

    let use_log_mode = opts.wants_commit_meta && rev.has_revision;

    let patch = if use_log_mode {
        run_git_command(&build_log_args(opts, &rev))?
    } else {
        run_git_command(&build_diff_args(opts, &rev, true))?
    };

    let untracked_paths = if opts.untracked && !use_log_mode {
        list_untracked_files(&opts.passthrough)?
    } else {
        Vec::new()
    };

    Ok(GitOutput::UnifiedDiff {
        patch,
        untracked_paths,
    })
}

fn build_diff_args(
    opts: &GitOptions,
    rev: &RevisionInfo,
    enforce_clean_patch: bool,
) -> Vec<String> {
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

    if opts.all_uncommitted && !rev.has_revision {
        args.push("HEAD".to_string());
    }

    args.extend(opts.passthrough.iter().cloned());
    args
}

fn build_log_args(opts: &GitOptions, rev: &RevisionInfo) -> Vec<String> {
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
        "--format=BETTER_GIT_DIFF_META:%h%x00%an%x00%ad%x00%s".to_string(),
    ];

    if !rev.is_range && !rev.has_limit_flag {
        args.push("-n".to_string());
        args.push("1".to_string());
    }

    args.extend(opts.passthrough.iter().cloned());
    args
}

fn list_untracked_files(passthrough: &[String]) -> Result<Vec<String>, AppError> {
    let mut args = vec![
        "--no-pager".to_string(),
        "-c".to_string(),
        "core.quotepath=false".to_string(),
        "ls-files".to_string(),
        "--others".to_string(),
        "--exclude-standard".to_string(),
    ];

    let pathspecs = extract_pathspecs_for_ls(passthrough);
    if !pathspecs.is_empty() {
        args.push("--".to_string());
        args.extend(pathspecs);
    }

    let output = run_git_command(&args)?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn extract_pathspecs_for_ls(args: &[String]) -> Vec<String> {
    let mut after_sep = false;
    let mut specs = Vec::new();
    for arg in args {
        if arg == "--" {
            after_sep = true;
            continue;
        }
        if after_sep || (!arg.starts_with('-') && Path::new(arg).exists()) {
            specs.push(arg.clone());
        }
    }
    specs
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
    args.iter().any(|arg| {
        PASSTHROUGH_SUMMARY_FLAGS
            .iter()
            .any(|flag| arg == flag || arg.starts_with("--stat="))
    })
}

fn inspect_revisions(args: &[String]) -> RevisionInfo {
    let mut info = RevisionInfo::default();
    let mut after_sep = false;

    for arg in args {
        if arg == "--" {
            after_sep = true;
            continue;
        }
        if after_sep {
            continue;
        }
        if arg.starts_with('-') {
            if arg == "-n"
                || arg.starts_with("-n")
                || arg == "--max-count"
                || arg.starts_with("--max-count=")
            {
                info.has_limit_flag = true;
            }
            continue;
        }
        if arg.contains("..") {
            info.has_revision = true;
            info.is_range = true;
            continue;
        }
        if !Path::new(arg).exists() && verify_git_commit_ish(arg) {
            info.has_revision = true;
        }
    }

    info
}

fn verify_git_commit_ish(candidate: &str) -> bool {
    Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{candidate}^{{commit}}"),
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
