use crate::cli::FilterOptions;
use std::path::Path;

const GENERATED_EXACT_NAMES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    "composer.lock",
    "Gemfile.lock",
    "Pipfile.lock",
    "poetry.lock",
    "uv.lock",
    "go.sum",
    "flake.lock",
    "packages.lock.json",
];

const GENERATED_SUFFIXES: &[&str] = &[
    ".lock",
    "-lock.json",
    "-lock.yaml",
    ".min.js",
    ".min.css",
    ".bundle.js",
    ".bundle.css",
    ".chunk.js",
    ".map",
    ".tsbuildinfo",
];

pub struct FileFilter<'a> {
    options: &'a FilterOptions,
}

impl<'a> FileFilter<'a> {
    pub fn new(options: &'a FilterOptions) -> Self {
        Self { options }
    }

    pub fn should_skip(&self, repo_rel_path: &str, is_binary: bool) -> bool {
        if self.options.include_generated {
            return false;
        }

        // Never skip a file the user explicitly targeted on the command line
        if self.is_explicitly_requested(repo_rel_path) {
            return false;
        }

        if is_binary {
            return true;
        }

        let file_name = Path::new(repo_rel_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(repo_rel_path);

        if GENERATED_EXACT_NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(file_name))
        {
            return true;
        }

        let lower_name = file_name.to_ascii_lowercase();
        if GENERATED_SUFFIXES
            .iter()
            .any(|suffix| lower_name.ends_with(suffix))
        {
            return true;
        }

        self.options
            .custom_excludes
            .iter()
            .any(|pattern| glob_matches(pattern, repo_rel_path) || glob_matches(pattern, file_name))
    }

    fn is_explicitly_requested(&self, repo_rel_path: &str) -> bool {
        let normalized_target = repo_rel_path.replace('\\', "/");
        self.options.explicit_paths.iter().any(|p| {
            let norm = p.trim_start_matches("./").trim_start_matches(".\\").replace('\\', "/");
            norm == normalized_target
        })
    }
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (None, 0);

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }

    pi == p.len()
}
