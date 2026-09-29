use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoContext {
    pub root: PathBuf,
    pub branch: Option<String>,
    pub head: String,
    pub default_branch: String,
}

impl RepoContext {
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repo")
            .to_string()
    }

    pub fn branch_label(&self) -> String {
        match &self.branch {
            Some(branch) => branch.clone(),
            None => format!("detached@{}", self.head),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewTarget {
    WorkingTree,
    Commit { rev: String },
    Range { base: String, head: String },
    PullRequest {
        number: u64,
        title: String,
        base_ref: String,
        head_ref: String,
    },
}

impl ReviewTarget {
    pub fn label(&self) -> String {
        match self {
            Self::WorkingTree => "working tree".to_string(),
            Self::Commit { rev } => format!("commit {}", short_sha(rev)),
            Self::Range { base, head } => format!("{base}...{head}"),
            Self::PullRequest { number, title, .. } => {
                if title.is_empty() {
                    format!("PR #{number}")
                } else {
                    format!("PR #{number} {title}")
                }
            }
        }
    }
}

fn short_sha(rev: &str) -> &str {
    if rev.len() > 12 {
        rev.get(..7).unwrap_or(rev)
    } else {
        rev
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Typechange,
}

impl FileStatus {
    pub fn glyph(self) -> char {
        match self {
            Self::Added => 'A',
            Self::Modified => 'M',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Copied => 'C',
            Self::Untracked => '?',
            Self::Typechange => 'T',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Staged,
    Unstaged,
    Both,
    Untracked,
    Committed,
}

impl ChangeKind {
    pub fn marker(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::Both => "mixed",
            Self::Untracked => "untracked",
            Self::Committed => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Meta,
    Hunk,
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub kind: ChangeKind,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
    pub lines: Vec<DiffLine>,
}

impl FileDiff {
    pub fn display_path(&self) -> String {
        match &self.old_path {
            Some(old) if old != &self.path => format!("{old} -> {}", self.path),
            _ => self.path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub sha: String,
    pub short_sha: String,
    pub author: String,
    pub date: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSummary {
    pub name: String,
    pub is_head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub base_ref: String,
    pub head_ref: String,
    pub url: String,
    pub is_draft: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Commits,
    Branches,
    PullRequests,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewState {
    pub repo: RepoContext,
    pub target: ReviewTarget,
    pub files: Vec<FileDiff>,
    pub selected: usize,
    pub scroll: usize,
    pub overlay: Overlay,
    pub overlay_selected: usize,
    pub commits: Vec<CommitSummary>,
    pub branches: Vec<BranchSummary>,
    pub prs: Vec<PullRequest>,
    pub status: String,
    pub pending_base: Option<String>,
}

impl ViewState {
    pub fn new(repo: RepoContext) -> Self {
        Self {
            repo,
            target: ReviewTarget::WorkingTree,
            files: Vec::new(),
            selected: 0,
            scroll: 0,
            overlay: Overlay::None,
            overlay_selected: 0,
            commits: Vec::new(),
            branches: Vec::new(),
            prs: Vec::new(),
            status: String::new(),
            pending_base: None,
        }
    }

    pub fn selected_file(&self) -> Option<&FileDiff> {
        self.files.get(self.selected)
    }

    pub fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let next = (self.selected + 1).min(self.files.len() - 1);
        if next != self.selected {
            self.selected = next;
            self.scroll = 0;
        }
    }

    pub fn prev_file(&mut self) {
        let next = self.selected.saturating_sub(1);
        if next != self.selected {
            self.selected = next;
            self.scroll = 0;
        }
    }

    pub fn scroll_by(&mut self, delta: isize) {
        if delta < 0 {
            self.scroll = self.scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.scroll = self.scroll.saturating_add(delta as usize);
        }
    }

    pub fn set_files(&mut self, files: Vec<FileDiff>) {
        let path = self.selected_file().map(|file| file.path.clone());
        self.files = files;
        self.selected = path
            .and_then(|path| self.files.iter().position(|file| file.path == path))
            .unwrap_or(0);
        if self.selected >= self.files.len() {
            self.selected = 0;
        }
        self.scroll = 0;
    }

    pub fn stat_summary(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(adds, dels), file| {
                (adds + file.additions, dels + file.deletions)
            })
    }

    pub fn overlay_len(&self) -> usize {
        match self.overlay {
            Overlay::None | Overlay::Help => 0,
            Overlay::Commits => self.commits.len(),
            Overlay::Branches => self.branches.len(),
            Overlay::PullRequests => self.prs.len(),
        }
    }

    pub fn overlay_next(&mut self) {
        let len = self.overlay_len();
        if len == 0 {
            self.overlay_selected = 0;
            return;
        }
        self.overlay_selected = (self.overlay_selected + 1).min(len - 1);
    }

    pub fn overlay_prev(&mut self) {
        self.overlay_selected = self.overlay_selected.saturating_sub(1);
    }

    pub fn open_overlay(&mut self, overlay: Overlay) {
        self.overlay = overlay;
        self.overlay_selected = 0;
    }

    pub fn close_overlay(&mut self) {
        self.overlay = Overlay::None;
        self.overlay_selected = 0;
    }
}
