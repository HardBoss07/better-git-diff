use crate::error::AppError;
use crate::filter::FileFilter;
use crate::model::{
    CommitMeta, CommitSection, DiffDocument, DiffLine, FileDiff, FileStatus, Hunk, LineKind,
};
use std::fs;

const META_PREFIX: &str = "BETTER_GIT_DIFF_META:";
const BINARY_SNIFF_LEN: usize = 8000;

pub fn parse_diff(
    raw: &str,
    untracked_paths: &[String],
    filter: &FileFilter<'_>,
) -> Result<DiffDocument, AppError> {
    let mut doc = DiffDocument::default();
    let mut current_commit = CommitSection::default();
    let mut active_file: Option<FileBuilder> = None;
    let mut skipping_current_file = false;

    for raw_line in raw.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if let Some(payload) = line.strip_prefix(META_PREFIX) {
            flush_file(
                &mut active_file,
                &mut current_commit,
                filter,
                &mut doc.skipped_files,
            );
            if current_commit.meta.is_some() || !current_commit.files.is_empty() {
                doc.commits.push(current_commit);
            }
            current_commit = CommitSection {
                meta: parse_meta_line(payload),
                files: Vec::new(),
            };
            skipping_current_file = false;
            continue;
        }

        if line.starts_with("diff --git ") {
            flush_file(
                &mut active_file,
                &mut current_commit,
                filter,
                &mut doc.skipped_files,
            );

            let (old_guess, new_guess) = parse_diff_git_paths(line);
            let target = if new_guess != "/dev/null" {
                &new_guess
            } else {
                &old_guess
            };

            if filter.should_skip(target, false) {
                doc.skipped_files.push(target.clone());
                skipping_current_file = true;
                active_file = None;
            } else {
                skipping_current_file = false;
                active_file = Some(FileBuilder::new(old_guess, new_guess, line));
            }
            continue;
        }

        if skipping_current_file {
            continue;
        }

        if let Some(builder) = active_file.as_mut() {
            builder.feed_line(line);
        }
    }

    flush_file(
        &mut active_file,
        &mut current_commit,
        filter,
        &mut doc.skipped_files,
    );

    for path in untracked_paths {
        append_untracked_file(path, filter, &mut current_commit, &mut doc.skipped_files)?;
    }

    if current_commit.meta.is_some() || !current_commit.files.is_empty() {
        doc.commits.push(current_commit);
    }

    Ok(doc)
}

fn append_untracked_file(
    path: &str,
    filter: &FileFilter<'_>,
    commit: &mut CommitSection,
    skipped: &mut Vec<String>,
) -> Result<(), AppError> {
    if filter.should_skip(path, false) {
        skipped.push(path.to_string());
        return Ok(());
    }

    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return Ok(()),
    };

    let sniff_end = bytes.len().min(BINARY_SNIFF_LEN);
    let is_binary = bytes[..sniff_end].contains(&0);

    if is_binary {
        if filter.should_skip(path, true) {
            skipped.push(path.to_string());
        } else {
            commit.files.push(FileDiff {
                old_path: "/dev/null".to_string(),
                new_path: path.to_string(),
                status: FileStatus::Binary,
                raw_headers: vec![
                    format!("diff --git a/{path} b/{path}"),
                    "new file mode 100644".to_string(),
                    format!("Binary files /dev/null and b/{path} differ"),
                ],
                hunks: Vec::new(),
            });
        }
        return Ok(());
    }

    let text = String::from_utf8_lossy(&bytes);
    let mut lines = Vec::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let content = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        lines.push(DiffLine {
            kind: LineKind::Addition,
            line_number: idx + 1,
            content: content.to_string(),
        });
    }

    let count = lines.len();
    let hunks = if count == 0 {
        Vec::new()
    } else {
        vec![Hunk {
            header: format!("@@ -0,0 +1,{count} @@"),
            old_start: 0,
            new_start: 1,
            lines,
        }]
    };

    commit.files.push(FileDiff {
        old_path: "/dev/null".to_string(),
        new_path: path.to_string(),
        status: FileStatus::Added,
        raw_headers: vec![
            format!("diff --git a/{path} b/{path}"),
            "new file mode 100644".to_string(),
            "--- /dev/null".to_string(),
            format!("+++ b/{path}"),
        ],
        hunks,
    });

    Ok(())
}

fn flush_file(
    active_file: &mut Option<FileBuilder>,
    commit: &mut CommitSection,
    filter: &FileFilter<'_>,
    skipped: &mut Vec<String>,
) {
    if let Some(builder) = active_file.take() {
        let file = builder.build();
        let is_binary = file.status == FileStatus::Binary;
        let check_path = if file.status == FileStatus::Deleted {
            &file.old_path
        } else {
            &file.new_path
        };

        if filter.should_skip(check_path, is_binary) {
            skipped.push(check_path.clone());
        } else {
            commit.files.push(file);
        }
    }
}

fn parse_meta_line(payload: &str) -> Option<CommitMeta> {
    let mut parts = payload.splitn(4, '\0');
    Some(CommitMeta {
        hash: parts.next()?.to_string(),
        author: parts.next().unwrap_or_default().to_string(),
        date: parts.next().unwrap_or_default().to_string(),
        message: parts.next().unwrap_or_default().to_string(),
    })
}

fn parse_diff_git_paths(line: &str) -> (String, String) {
    let rest = line.strip_prefix("diff --git ").unwrap_or("");
    if rest.len() > 2 && rest.len() % 2 == 1 {
        let mid = rest.len() / 2;
        if rest.as_bytes().get(mid) == Some(&b' ') {
            let (left, right_with_space) = rest.split_at(mid);
            let right = &right_with_space[1..];
            if let (Some(a), Some(b)) = (left.strip_prefix("a/"), right.strip_prefix("b/")) {
                if a == b {
                    return (a.to_string(), b.to_string());
                }
            }
        }
    }
    if let Some((a_part, b_part)) = rest.split_once(" b/") {
        let old_path = a_part.strip_prefix("a/").unwrap_or(a_part).to_string();
        return (old_path, b_part.to_string());
    }
    (rest.to_string(), rest.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawStatusKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Binary,
}

struct FileBuilder {
    old_path: String,
    new_path: String,
    status_kind: RawStatusKind,
    similarity: Option<u8>,
    raw_headers: Vec<String>,
    hunks: Vec<Hunk>,
    current_hunk: Option<Hunk>,
    old_line: usize,
    new_line: usize,
    in_hunks: bool,
}

impl FileBuilder {
    fn new(old_path: String, new_path: String, first_header: &str) -> Self {
        Self {
            old_path,
            new_path,
            status_kind: RawStatusKind::Modified,
            similarity: None,
            raw_headers: vec![first_header.to_string()],
            hunks: Vec::new(),
            current_hunk: None,
            old_line: 0,
            new_line: 0,
            in_hunks: false,
        }
    }

    fn feed_line(&mut self, line: &str) {
        if line.starts_with("@@ ") {
            if let Some(hunk) = self.current_hunk.take() {
                self.hunks.push(hunk);
            }
            self.in_hunks = true;
            let (old_start, new_start) = parse_hunk_starts(line);
            self.old_line = old_start;
            self.new_line = new_start;
            self.current_hunk = Some(Hunk {
                header: line.to_string(),
                old_start,
                new_start,
                lines: Vec::new(),
            });
            return;
        }

        if !self.in_hunks {
            self.raw_headers.push(line.to_string());
            self.parse_extended_header(line);
            return;
        }

        if let Some(hunk) = self.current_hunk.as_mut() {
            if let Some(content) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::Addition,
                    line_number: self.new_line,
                    content: content.to_string(),
                });
                self.new_line += 1;
            } else if let Some(content) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::Deletion,
                    line_number: self.old_line,
                    content: content.to_string(),
                });
                self.old_line += 1;
            } else if let Some(content) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::Context,
                    line_number: self.new_line,
                    content: content.to_string(),
                });
                self.old_line += 1;
                self.new_line += 1;
            } else if line.starts_with('\\') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::NoNewline,
                    line_number: 0,
                    content: line.to_string(),
                });
            }
        }
    }

    fn parse_extended_header(&mut self, line: &str) {
        if line.starts_with("new file mode ") {
            self.status_kind = RawStatusKind::Added;
        } else if line.starts_with("deleted file mode ") {
            self.status_kind = RawStatusKind::Deleted;
        } else if line.starts_with("old mode ") || line.starts_with("new mode ") {
            if self.status_kind == RawStatusKind::Modified {
                self.status_kind = RawStatusKind::TypeChanged;
            }
        } else if let Some(pct_str) = line.strip_prefix("similarity index ") {
            self.similarity = pct_str.trim_end_matches('%').parse::<u8>().ok();
        } else if let Some(from) = line.strip_prefix("rename from ") {
            self.old_path = from.to_string();
            self.status_kind = RawStatusKind::Renamed;
        } else if let Some(to) = line.strip_prefix("rename to ") {
            self.new_path = to.to_string();
            self.status_kind = RawStatusKind::Renamed;
        } else if let Some(from) = line.strip_prefix("copy from ") {
            self.old_path = from.to_string();
            self.status_kind = RawStatusKind::Copied;
        } else if let Some(to) = line.strip_prefix("copy to ") {
            self.new_path = to.to_string();
            self.status_kind = RawStatusKind::Copied;
        } else if let Some(old_p) = line.strip_prefix("--- ") {
            if old_p == "/dev/null" {
                self.status_kind = RawStatusKind::Added;
                self.old_path = "/dev/null".to_string();
            } else {
                self.old_path = old_p.strip_prefix("a/").unwrap_or(old_p).to_string();
            }
        } else if let Some(new_p) = line.strip_prefix("+++ ") {
            if new_p == "/dev/null" {
                self.status_kind = RawStatusKind::Deleted;
                self.new_path = "/dev/null".to_string();
            } else {
                self.new_path = new_p.strip_prefix("b/").unwrap_or(new_p).to_string();
            }
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            self.status_kind = RawStatusKind::Binary;
        }
    }

    fn build(mut self) -> FileDiff {
        if let Some(hunk) = self.current_hunk.take() {
            self.hunks.push(hunk);
        }
        let status = match self.status_kind {
            RawStatusKind::Modified => FileStatus::Modified,
            RawStatusKind::Added => FileStatus::Added,
            RawStatusKind::Deleted => FileStatus::Deleted,
            RawStatusKind::Renamed => FileStatus::Renamed(self.similarity),
            RawStatusKind::Copied => FileStatus::Copied(self.similarity),
            RawStatusKind::TypeChanged => FileStatus::TypeChanged,
            RawStatusKind::Binary => FileStatus::Binary,
        };
        FileDiff {
            old_path: self.old_path,
            new_path: self.new_path,
            status,
            raw_headers: self.raw_headers,
            hunks: self.hunks,
        }
    }
}

fn parse_hunk_starts(header: &str) -> (usize, usize) {
    let inner = header
        .strip_prefix("@@ ")
        .and_then(|s| s.split_once(" @@"))
        .map(|(range, _)| range)
        .unwrap_or("");

    let mut old_start = 1;
    let mut new_start = 1;

    for token in inner.split_whitespace() {
        if let Some(old_part) = token.strip_prefix('-') {
            let num = old_part.split(',').next().unwrap_or("1");
            old_start = num.parse().unwrap_or(1);
        } else if let Some(new_part) = token.strip_prefix('+') {
            let num = new_part.split(',').next().unwrap_or("1");
            new_start = num.parse().unwrap_or(1);
        }
    }

    (old_start, new_start)
}
