use crate::filter::FileFilter;
use crate::model::{
    CommitMeta, CommitSection, DiffDocument, DiffLine, FileDiff, FileStatus, Hunk, LineKind,
};

const META_PREFIX: &str = "BGD_META:";

pub fn parse_diff(raw: &str, filter: &FileFilter<'_>) -> DiffDocument {
    let mut doc = DiffDocument::default();
    let mut current_commit = CommitSection::default();
    let mut active_file: Option<FileBuilder> = None;
    let mut skipping_current_file = false;

    for raw_line in raw.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if let Some(payload) = line.strip_prefix(META_PREFIX) {
            flush_file(&mut active_file, &mut current_commit, filter, &mut doc.skipped_files);
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
            flush_file(&mut active_file, &mut current_commit, filter, &mut doc.skipped_files);

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

    flush_file(&mut active_file, &mut current_commit, filter, &mut doc.skipped_files);
    if current_commit.meta.is_some() || !current_commit.files.is_empty() {
        doc.commits.push(current_commit);
    }

    doc
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
    if let Some((a_part, b_part)) = rest.split_once(" b/") {
        let old_path = a_part.strip_prefix("a/").unwrap_or(a_part).to_string();
        return (old_path, b_part.to_string());
    }
    (rest.to_string(), rest.to_string())
}

struct FileBuilder {
    old_path: String,
    new_path: String,
    status: FileStatus,
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
            status: FileStatus::Modified,
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
            }
        }
    }

    fn parse_extended_header(&mut self, line: &str) {
        if line.starts_with("new file mode ") {
            self.status = FileStatus::Added;
        } else if line.starts_with("deleted file mode ") {
            self.status = FileStatus::Deleted;
        } else if line.starts_with("old mode ") || line.starts_with("new mode ") {
            if self.status == FileStatus::Modified {
                self.status = FileStatus::TypeChanged;
            }
        } else if let Some(pct_str) = line.strip_prefix("similarity index ") {
            let pct = pct_str.trim_end_matches('%').parse::<u8>().ok();
            match self.status {
                FileStatus::Renamed(_) => self.status = FileStatus::Renamed(pct),
                FileStatus::Copied(_) => self.status = FileStatus::Copied(pct),
                _ => {}
            }
        } else if let Some(from) = line.strip_prefix("rename from ") {
            self.old_path = from.to_string();
            self.status = FileStatus::Renamed(None);
        } else if let Some(to) = line.strip_prefix("rename to ") {
            self.new_path = to.to_string();
            if !matches!(self.status, FileStatus::Renamed(_)) {
                self.status = FileStatus::Renamed(None);
            }
        } else if let Some(from) = line.strip_prefix("copy from ") {
            self.old_path = from.to_string();
            self.status = FileStatus::Copied(None);
        } else if let Some(to) = line.strip_prefix("copy to ") {
            self.new_path = to.to_string();
            if !matches!(self.status, FileStatus::Copied(_)) {
                self.status = FileStatus::Copied(None);
            }
        } else if let Some(old_p) = line.strip_prefix("--- ") {
            if old_p == "/dev/null" {
                self.status = FileStatus::Added;
                self.old_path = "/dev/null".to_string();
            } else {
                self.old_path = old_p.strip_prefix("a/").unwrap_or(old_p).to_string();
            }
        } else if let Some(new_p) = line.strip_prefix("+++ ") {
            if new_p == "/dev/null" {
                self.status = FileStatus::Deleted;
                self.new_path = "/dev/null".to_string();
            } else {
                self.new_path = new_p.strip_prefix("b/").unwrap_or(new_p).to_string();
            }
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            self.status = FileStatus::Binary;
        }
    }

    fn build(mut self) -> FileDiff {
        if let Some(hunk) = self.current_hunk.take() {
            self.hunks.push(hunk);
        }
        FileDiff {
            old_path: self.old_path,
            new_path: self.new_path,
            status: self.status,
            raw_headers: self.raw_headers,
            hunks: self.hunks,
        }
    }
}

fn parse_hunk_starts(header: &str) -> (usize, usize) {
    // Format: @@ -old_start[,old_count] +new_start[,new_count] @@ ...
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
