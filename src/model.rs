#[derive(Debug, Default)]
pub struct DiffDocument {
    pub commits: Vec<CommitSection>,
    pub skipped_files: Vec<String>,
}

impl DiffDocument {
    pub fn is_empty(&self) -> bool {
        self.commits.iter().all(|c| c.files.is_empty()) && self.skipped_files.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct CommitSection {
    pub meta: Option<CommitMeta>,
    pub files: Vec<FileDiff>,
}

#[derive(Debug, Clone)]
pub struct CommitMeta {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed(Option<u8>),
    Copied(Option<u8>),
    TypeChanged,
    Binary,
}

impl FileStatus {
    pub fn badge(&self) -> String {
        match self {
            FileStatus::Modified => "M".to_string(),
            FileStatus::Added => "A".to_string(),
            FileStatus::Deleted => "D".to_string(),
            FileStatus::Renamed(Some(pct)) => format!("R {pct}%"),
            FileStatus::Renamed(None) => "R".to_string(),
            FileStatus::Copied(Some(pct)) => format!("C {pct}%"),
            FileStatus::Copied(None) => "C".to_string(),
            FileStatus::TypeChanged => "T".to_string(),
            FileStatus::Binary => "B".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub status: FileStatus,
    pub raw_headers: Vec<String>,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    pub fn display_path(&self) -> String {
        match self.status {
            FileStatus::Renamed(_) | FileStatus::Copied(_) if self.old_path != self.new_path => {
                format!("{} -> {}", self.old_path, self.new_path)
            }
            FileStatus::Deleted => self.old_path.clone(),
            _ => self.new_path.clone(),
        }
    }

    pub fn max_line_number(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .map(|l| l.line_number)
            .max()
            .unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Addition,
    Deletion,
    Context,
}

#[derive(Debug)]
pub struct DiffLine {
    pub kind: LineKind,
    pub line_number: usize,
    pub content: String,
}
