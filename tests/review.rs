use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use gitcrack::git;
use gitcrack::model::{ChangeKind, DiffLineKind, FileStatus};

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("gitcrack-review-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn git_cmd(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn review_working_tree_commit_and_range() {
    let repo_dir = TempRepo::new();
    let dir = &repo_dir.path;
    git_cmd(dir, &["init", "-b", "main"]);
    git_cmd(dir, &["config", "user.name", "Test User"]);
    git_cmd(dir, &["config", "user.email", "test@example.com"]);

    fs::write(dir.join("a.txt"), "alpha\nbeta\n").unwrap();
    fs::write(dir.join("b.txt"), "keep\n").unwrap();
    git_cmd(dir, &["add", "a.txt", "b.txt"]);
    git_cmd(dir, &["commit", "-m", "init"]);

    let repo = git::discover(dir).unwrap();
    assert_eq!(repo.branch.as_deref(), Some("main"));
    assert_ne!(repo.head, "unborn");
    assert_eq!(repo.default_branch, "main");

    let committed = git::commit_diff(&repo, "HEAD").unwrap();
    assert!(
        committed
            .iter()
            .all(|file| file.kind == ChangeKind::Committed)
    );
    let added = committed.iter().find(|file| file.path == "a.txt").unwrap();
    assert_eq!(added.status, FileStatus::Added);
    assert_eq!(added.additions, 2);
    assert!(committed.iter().any(|file| file.path == "b.txt"));

    let commits = git::list_commits(&repo, 0).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].subject, "init");
    assert_eq!(git::commit_diff(&repo, &commits[0].sha).unwrap().len(), 2);

    fs::write(dir.join("a.txt"), "alpha\nchanged\n").unwrap();
    fs::write(dir.join("c.txt"), "fresh\nline\n").unwrap();
    fs::write(dir.join("d.txt"), "staged\n").unwrap();
    fs::write(dir.join("bin.dat"), [0, 255, 1]).unwrap();
    fs::write(dir.join("big.txt"), vec![b'a'; 200_001]).unwrap();
    git_cmd(dir, &["add", "d.txt"]);
    git_cmd(dir, &["mv", "b.txt", "b-renamed.txt"]);

    let work = git::working_tree(&repo).unwrap();
    assert!(work.windows(2).all(|pair| pair[0].path <= pair[1].path));

    let modified = work.iter().find(|file| file.path == "a.txt").unwrap();
    assert_eq!(modified.status, FileStatus::Modified);
    assert_eq!(modified.kind, ChangeKind::Unstaged);
    assert_eq!(modified.additions, 1);
    assert_eq!(modified.deletions, 1);
    assert!(modified.lines.iter().any(|line| {
        line.kind == DiffLineKind::Add && line.text == "changed" && line.new_lineno == Some(2)
    }));

    let renamed = work
        .iter()
        .find(|file| file.path == "b-renamed.txt")
        .unwrap();
    assert_eq!(renamed.status, FileStatus::Renamed);
    assert_eq!(renamed.kind, ChangeKind::Staged);
    assert_eq!(renamed.old_path.as_deref(), Some("b.txt"));

    let untracked = work.iter().find(|file| file.path == "c.txt").unwrap();
    assert_eq!(untracked.status, FileStatus::Untracked);
    assert_eq!(untracked.kind, ChangeKind::Untracked);
    assert_eq!(untracked.additions, 2);
    assert_eq!(untracked.lines[0].kind, DiffLineKind::Add);
    assert_eq!(untracked.lines[0].text, "fresh");
    assert_eq!(untracked.lines[0].new_lineno, Some(1));
    assert_eq!(untracked.lines[1].new_lineno, Some(2));

    let staged = work.iter().find(|file| file.path == "d.txt").unwrap();
    assert_eq!(staged.status, FileStatus::Added);
    assert_eq!(staged.kind, ChangeKind::Staged);
    assert_eq!(staged.additions, 1);

    let binary = work.iter().find(|file| file.path == "bin.dat").unwrap();
    assert!(binary.binary);
    assert_eq!(binary.kind, ChangeKind::Untracked);
    assert_eq!(binary.lines.len(), 1);
    assert_eq!(binary.lines[0].kind, DiffLineKind::Meta);

    let big = work.iter().find(|file| file.path == "big.txt").unwrap();
    assert!(big.binary);
    assert_eq!(big.additions, 0);

    git_cmd(dir, &["reset", "--hard"]);
    git_cmd(dir, &["clean", "-fd"]);
    git_cmd(dir, &["checkout", "-b", "feature"]);
    fs::write(dir.join("a.txt"), "alpha\nbeta\nfrom-feature\n").unwrap();
    git_cmd(dir, &["add", "a.txt"]);
    git_cmd(dir, &["commit", "-m", "feature change"]);
    git_cmd(dir, &["checkout", "main"]);
    fs::write(dir.join("b.txt"), "keep\nfrom-main\n").unwrap();
    git_cmd(dir, &["add", "b.txt"]);
    git_cmd(dir, &["commit", "-m", "main change"]);

    let range = git::range_diff(&repo, "main", "feature").unwrap();
    assert!(range.iter().all(|file| file.kind == ChangeKind::Committed));
    assert!(
        range
            .iter()
            .any(|file| file.path == "a.txt" && file.status == FileStatus::Modified)
    );
    assert!(range.iter().all(|file| file.path != "b.txt"));

    let branches = git::list_branches(&repo).unwrap();
    assert_eq!(branches[0].name, "main");
    assert!(branches[0].is_head);
    assert!(
        branches
            .iter()
            .any(|branch| branch.name == "feature" && !branch.is_head)
    );
}

#[test]
fn discover_rejects_non_repo() {
    let dir = TempRepo::new();
    let err = git::discover(&dir.path).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("not a git repository"));
    assert!(message.contains(&dir.path.display().to_string()));
}
