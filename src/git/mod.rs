mod parse;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::Result;

use crate::model::{
    BranchSummary, ChangeKind, CommitSummary, DiffLine, DiffLineKind, FileDiff, FileStatus,
    RepoContext,
};

pub use parse::parse_unified_diff;

pub fn discover(cwd: &Path) -> Result<RepoContext> {
    let output = git(cwd, &["rev-parse", "--show-toplevel"])?;
    if !output.status.success() {
        anyhow::bail!("not a git repository (starting from {})", cwd.display());
    }
    let root_text = String::from_utf8_lossy(&output.stdout);
    let root_text = root_text.trim();
    if root_text.is_empty() {
        anyhow::bail!("not a git repository (starting from {})", cwd.display());
    }
    let root = PathBuf::from(root_text);
    let branch_text = git_ok(&root, &["branch", "--show-current"])?;
    let branch_text = branch_text.trim();
    let branch = if branch_text.is_empty() {
        None
    } else {
        Some(branch_text.to_string())
    };
    let head = match git(&root, &["rev-parse", "--short", "HEAD"]) {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let text = text.trim();
            if text.is_empty() {
                "unborn".to_string()
            } else {
                text.to_string()
            }
        }
        _ => "unborn".to_string(),
    };
    let default_branch = default_branch(&root, branch.as_deref());
    Ok(RepoContext {
        root,
        branch,
        head,
        default_branch,
    })
}

pub fn working_tree(repo: &RepoContext) -> Result<Vec<FileDiff>> {
    let mut files = if head_exists(&repo.root)? {
        let text = git_ok(
            &repo.root,
            &[
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--find-renames",
                "-U3",
                "HEAD",
            ],
        )?;
        parse_unified_diff(&text)
    } else {
        Vec::new()
    };
    let status = git_ok(&repo.root, &["status", "--porcelain=v1", "-z", "-uall"])?;
    for entry in parse_porcelain(&status) {
        if let Some(file) = files.iter_mut().find(|file| file.path == entry.path) {
            file.kind = entry.kind;
            file.status = entry.status;
        } else if entry.kind == ChangeKind::Untracked {
            files.push(synthesize_untracked(&repo.root, &entry.path));
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

pub fn commit_diff(repo: &RepoContext, rev: &str) -> Result<Vec<FileDiff>> {
    let output = git(
        &repo.root,
        &[
            "show",
            "--no-ext-diff",
            "--no-color",
            "--format=",
            "--find-renames",
            "-U3",
            rev,
        ],
    )?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if missing_parent(&stderr) {
            return Ok(Vec::new());
        }
        anyhow::bail!("git show failed: {}", stderr.trim());
    }
    Ok(committed(parse_unified_diff(&String::from_utf8_lossy(
        &output.stdout,
    ))))
}

pub fn range_diff(repo: &RepoContext, base: &str, head: &str) -> Result<Vec<FileDiff>> {
    let spec = format!("{base}...{head}");
    let text = git_ok(
        &repo.root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--find-renames",
            "-U3",
            &spec,
        ],
    )?;
    Ok(committed(parse_unified_diff(&text)))
}

pub fn list_commits(repo: &RepoContext, limit: usize) -> Result<Vec<CommitSummary>> {
    if !head_exists(&repo.root)? {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 500);
    let limit = limit.to_string();
    let text = git_ok(
        &repo.root,
        &[
            "log",
            "-n",
            &limit,
            "--pretty=format:%H%x1f%h%x1f%an%x1f%ad%x1f%s",
            "--date=short",
        ],
    )?;
    let mut commits = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\u{1f}').collect();
        if parts.len() < 5 {
            continue;
        }
        commits.push(CommitSummary {
            sha: parts[0].to_string(),
            short_sha: parts[1].to_string(),
            author: parts[2].to_string(),
            date: parts[3].to_string(),
            subject: parts[4..].join("\u{1f}"),
        });
    }
    Ok(commits)
}

pub fn list_branches(repo: &RepoContext) -> Result<Vec<BranchSummary>> {
    let text = git_ok(
        &repo.root,
        &[
            "for-each-ref",
            "--format=%(refname:short)%09%(HEAD)",
            "refs/heads",
        ],
    )?;
    let mut branches = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (name, head) = line.split_once('\t').unwrap_or((line, ""));
        if name.is_empty() {
            continue;
        }
        branches.push(BranchSummary {
            name: name.to_string(),
            is_head: head.trim() == "*",
        });
    }
    branches.sort_by(|left, right| {
        right
            .is_head
            .cmp(&left.is_head)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(branches)
}

fn committed(mut files: Vec<FileDiff>) -> Vec<FileDiff> {
    for file in &mut files {
        file.kind = ChangeKind::Committed;
    }
    files
}

fn missing_parent(stderr: &str) -> bool {
    stderr.to_ascii_lowercase().contains("parent")
}

fn head_exists(root: &Path) -> Result<bool> {
    let output = git(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    Ok(output.status.success())
}

fn default_branch(root: &Path, current: Option<&str>) -> String {
    if let Ok(output) = git(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let text = text.trim();
            if !text.is_empty() {
                if let Some((_, rest)) = text.split_once('/') {
                    if !rest.is_empty() {
                        return rest.to_string();
                    }
                }
                return text.to_string();
            }
        }
    }
    if ref_exists(root, "refs/heads/main") {
        return "main".to_string();
    }
    if ref_exists(root, "refs/heads/master") {
        return "master".to_string();
    }
    if let Some(branch) = current {
        if !branch.is_empty() {
            return branch.to_string();
        }
    }
    "main".to_string()
}

fn ref_exists(root: &Path, name: &str) -> bool {
    match git(root, &["show-ref", "--verify", "--quiet", name]) {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

struct PorcelainEntry {
    path: String,
    kind: ChangeKind,
    status: FileStatus,
}

fn parse_porcelain(data: &str) -> Vec<PorcelainEntry> {
    if data.contains('\0') {
        parse_porcelain_z(data)
    } else {
        data.lines()
            .filter(|line| !line.is_empty())
            .filter_map(parse_porcelain_line)
            .collect()
    }
}

fn parse_porcelain_z(data: &str) -> Vec<PorcelainEntry> {
    let parts: Vec<&str> = data.split('\0').collect();
    let mut entries = Vec::new();
    let mut index = 0;
    while index < parts.len() {
        let record = parts[index];
        if record.is_empty() {
            index += 1;
            continue;
        }
        if record.len() == 2 && index + 2 < parts.len() {
            let x = record.as_bytes()[0] as char;
            let y = record.as_bytes()[1] as char;
            if is_rename_or_copy(x, y)
                && !parts[index + 1].is_empty()
                && !parts[index + 2].is_empty()
            {
                let (kind, status) = classify_xy(x, y);
                entries.push(PorcelainEntry {
                    path: parts[index + 2].to_string(),
                    kind,
                    status,
                });
                index += 3;
                continue;
            }
        }
        if record.len() < 3 {
            index += 1;
            continue;
        }
        let x = record.as_bytes()[0] as char;
        let y = record.as_bytes()[1] as char;
        let rest = record[2..].strip_prefix(' ').unwrap_or(&record[2..]);
        let path = if is_rename_or_copy(x, y) {
            if index + 1 < parts.len() && !parts[index + 1].is_empty() {
                index += 1;
                rest.to_string()
            } else if let Some((_, new)) = split_arrow(rest) {
                new
            } else {
                rest.to_string()
            }
        } else {
            rest.to_string()
        };
        if path.is_empty() {
            index += 1;
            continue;
        }
        let (kind, status) = classify_xy(x, y);
        entries.push(PorcelainEntry { path, kind, status });
        index += 1;
    }
    entries
}

fn parse_porcelain_line(line: &str) -> Option<PorcelainEntry> {
    if line.len() < 3 {
        return None;
    }
    let x = line.as_bytes()[0] as char;
    let y = line.as_bytes()[1] as char;
    let rest = line[2..].strip_prefix(' ').unwrap_or(&line[2..]);
    let path = if is_rename_or_copy(x, y) {
        split_arrow(rest)
            .map(|(_, new)| new)
            .unwrap_or_else(|| rest.to_string())
    } else {
        rest.to_string()
    };
    if path.is_empty() {
        return None;
    }
    let (kind, status) = classify_xy(x, y);
    Some(PorcelainEntry { path, kind, status })
}

fn split_arrow(text: &str) -> Option<(String, String)> {
    let (old, new) = text.split_once(" -> ")?;
    let old = unquote_field(old.trim());
    let new = unquote_field(new.trim());
    if old.is_empty() || new.is_empty() {
        None
    } else {
        Some((old, new))
    }
}

fn unquote_field(text: &str) -> String {
    if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        let inner = &text[1..text.len() - 1];
        return inner.replace("\\\"", "\"").replace("\\\\", "\\");
    }
    text.to_string()
}

fn is_rename_or_copy(x: char, y: char) -> bool {
    matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C')
}

fn classify_xy(x: char, y: char) -> (ChangeKind, FileStatus) {
    if x == '?' && y == '?' {
        return (ChangeKind::Untracked, FileStatus::Untracked);
    }
    let kind = if x != ' ' && y != ' ' {
        ChangeKind::Both
    } else if x != ' ' {
        ChangeKind::Staged
    } else {
        ChangeKind::Unstaged
    };
    let letter = if y != ' ' { y } else { x };
    (kind, map_status(letter))
}

fn map_status(letter: char) -> FileStatus {
    match letter {
        'A' => FileStatus::Added,
        'M' => FileStatus::Modified,
        'D' => FileStatus::Deleted,
        'R' => FileStatus::Renamed,
        'C' => FileStatus::Copied,
        'T' => FileStatus::Typechange,
        '?' => FileStatus::Untracked,
        'U' => FileStatus::Modified,
        _ => FileStatus::Modified,
    }
}

fn synthesize_untracked(root: &Path, path: &str) -> FileDiff {
    let full = root.join(path);
    let Ok(meta) = std::fs::metadata(&full) else {
        return binary_untracked(path);
    };
    if !meta.is_file() || meta.len() > 200_000 {
        return binary_untracked(path);
    }
    let Ok(bytes) = std::fs::read(&full) else {
        return binary_untracked(path);
    };
    if bytes.len() > 200_000 {
        return binary_untracked(path);
    }
    match String::from_utf8(bytes) {
        Ok(text) => text_untracked(path, &text),
        Err(_) => binary_untracked(path),
    }
}

fn text_untracked(path: &str, text: &str) -> FileDiff {
    let mut lines = Vec::new();
    for (index, line) in split_file_lines(text).into_iter().enumerate() {
        lines.push(DiffLine {
            kind: DiffLineKind::Add,
            old_lineno: None,
            new_lineno: Some((index as u32).saturating_add(1)),
            text: line,
        });
    }
    FileDiff {
        path: path.to_string(),
        old_path: None,
        status: FileStatus::Untracked,
        kind: ChangeKind::Untracked,
        additions: lines.len() as u32,
        deletions: 0,
        binary: false,
        lines,
    }
}

fn split_file_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        parts.pop();
    }
    parts
        .into_iter()
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

fn binary_untracked(path: &str) -> FileDiff {
    FileDiff {
        path: path.to_string(),
        old_path: None,
        status: FileStatus::Untracked,
        kind: ChangeKind::Untracked,
        additions: 0,
        deletions: 0,
        binary: true,
        lines: vec![DiffLine {
            kind: DiffLineKind::Meta,
            old_lineno: None,
            new_lineno: None,
            text: "Binary file".to_string(),
        }],
    }
}

fn git(cwd: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run git {}: {err}", args.join(" ")))
}

fn git_ok(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = git(cwd, args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
