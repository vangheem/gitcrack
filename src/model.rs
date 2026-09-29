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
    Commit {
        rev: String,
    },
    Range {
        base: String,
        head: String,
    },
    PullRequest {
        number: u64,
        title: String,
        base_ref: String,
        head_ref: String,
    },
}

impl ReviewTarget {
    pub fn pr_number(&self) -> Option<u64> {
        match self {
            Self::PullRequest { number, .. } => Some(*number),
            _ => None,
        }
    }

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
    pub fn visible_len(&self) -> usize {
        if self.binary {
            self.lines
                .iter()
                .filter(|line| line.kind == DiffLineKind::Meta)
                .count()
        } else {
            self.lines.len()
        }
    }

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
    pub review: ReviewState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Reviewed,
    Unreviewed,
}

impl ReviewState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::ChangesRequested => "changes",
            Self::Reviewed => "reviewed",
            Self::Unreviewed => "unreviewed",
        }
    }

    pub fn from_reviews<'a>(decision: &str, states: impl IntoIterator<Item = &'a str>) -> Self {
        match decision {
            "APPROVED" => return Self::Approved,
            "CHANGES_REQUESTED" => return Self::ChangesRequested,
            _ => {}
        }
        let mut commented = false;
        for state in states {
            match state {
                "APPROVED" => return Self::Approved,
                "CHANGES_REQUESTED" => return Self::ChangesRequested,
                "COMMENTED" => commented = true,
                _ => {}
            }
        }
        if commented {
            Self::Reviewed
        } else {
            Self::Unreviewed
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewKind {
    Comment,
    Approve,
    RequestChanges,
}

impl ReviewKind {
    pub const CHOICES: [Self; 3] = [Self::Comment, Self::Approve, Self::RequestChanges];

    pub fn label(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::Approve => "approve",
            Self::RequestChanges => "request changes",
        }
    }

    pub fn flag(self) -> &'static str {
        match self {
            Self::Comment => "--comment",
            Self::Approve => "--approve",
            Self::RequestChanges => "--request-changes",
        }
    }

    pub fn needs_body(self) -> bool {
        !matches!(self, Self::Approve)
    }

    pub fn submitted(self) -> &'static str {
        match self {
            Self::Comment => "commented",
            Self::Approve => "approved",
            Self::RequestChanges => "changes requested",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    Pass,
    Fail,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Squash,
    Merge,
    Rebase,
}

impl MergeMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Squash => "squash",
            Self::Merge => "merge",
            Self::Rebase => "rebase",
        }
    }

    pub fn flag(self) -> &'static str {
        match self {
            Self::Squash => "--squash",
            Self::Merge => "--merge",
            Self::Rebase => "--rebase",
        }
    }

    pub fn pick(squash: bool, merge: bool, rebase: bool) -> Option<Self> {
        if squash {
            Some(Self::Squash)
        } else if merge {
            Some(Self::Merge)
        } else if rebase {
            Some(Self::Rebase)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeReadiness {
    pub ci_ok: bool,
    pub ci_label: String,
    pub merge_ok: bool,
    pub merge_label: String,
    pub method: Option<MergeMethod>,
    pub problems: Vec<String>,
}

impl MergeReadiness {
    pub fn can_merge(&self) -> bool {
        self.ci_ok && self.merge_ok && self.method.is_some()
    }

    pub fn assess(
        mergeable: &str,
        merge_state: &str,
        draft: bool,
        checks: &[(String, CheckResult)],
        method: Option<MergeMethod>,
    ) -> Self {
        let failing: Vec<_> = checks
            .iter()
            .filter(|(_, result)| *result == CheckResult::Fail)
            .map(|(name, _)| format!("{name} failing"))
            .collect();
        let pending: Vec<_> = checks
            .iter()
            .filter(|(_, result)| *result == CheckResult::Pending)
            .map(|(name, _)| format!("{name} pending"))
            .collect();
        let ci_ok = failing.is_empty() && pending.is_empty();
        let ci_label = if checks.is_empty() {
            "no checks".to_string()
        } else if !failing.is_empty() {
            format!("failing ({})", failing.len())
        } else if !pending.is_empty() {
            format!("pending ({})", pending.len())
        } else {
            "passing".to_string()
        };
        let merge_label = merge_block(mergeable, merge_state, draft, method)
            .unwrap_or_else(|| "allowed".to_string());
        let merge_ok = merge_label == "allowed";
        let mut problems = failing;
        problems.extend(pending);
        problems.truncate(6);
        Self {
            ci_ok,
            ci_label,
            merge_ok,
            merge_label,
            method,
            problems,
        }
    }
}

fn merge_block(
    mergeable: &str,
    merge_state: &str,
    draft: bool,
    method: Option<MergeMethod>,
) -> Option<String> {
    if draft || merge_state == "DRAFT" {
        return Some("draft".to_string());
    }
    if mergeable == "CONFLICTING" || merge_state == "DIRTY" {
        return Some("conflicts".to_string());
    }
    if merge_state == "BEHIND" {
        return Some("behind base".to_string());
    }
    if merge_state == "BLOCKED" {
        return Some("blocked".to_string());
    }
    if merge_state == "UNKNOWN" || mergeable == "UNKNOWN" {
        return Some("unknown".to_string());
    }
    if method.is_none() {
        return Some("no merge method".to_string());
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Commits,
    Branches,
    PullRequests,
    Help,
    Review,
    Merge,
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
    pub comment: String,
    pub comment_cursor: usize,
    pub comment_focused: bool,
    pub pr_review: Option<ReviewState>,
    pub merge: Option<MergeReadiness>,
    pub loading: Option<String>,
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
            comment: String::new(),
            comment_cursor: 0,
            comment_focused: false,
            pr_review: None,
            merge: None,
            loading: None,
        }
    }

    pub fn is_pull_request(&self) -> bool {
        self.target.pr_number().is_some()
    }

    pub fn comment_rows(&self) -> u16 {
        if !self.is_pull_request() {
            0
        } else if self.comment_focused {
            3
        } else {
            1
        }
    }

    pub fn review_choice(&self) -> ReviewKind {
        ReviewKind::CHOICES
            .get(self.overlay_selected)
            .copied()
            .unwrap_or(ReviewKind::Comment)
    }

    pub fn insert_comment(&mut self, ch: char) {
        let at = self.cursor_at();
        self.comment.insert(at, ch);
        self.comment_cursor = at + ch.len_utf8();
    }

    pub fn comment_backspace(&mut self) {
        let at = self.cursor_at();
        if at == 0 {
            self.comment_cursor = 0;
            return;
        }
        let prev = self.comment.floor_char_boundary(at - 1);
        self.comment.replace_range(prev..at, "");
        self.comment_cursor = prev;
    }

    pub fn comment_delete(&mut self) {
        let at = self.cursor_at();
        let Some(ch) = self.comment[at..].chars().next() else {
            self.comment_cursor = at;
            return;
        };
        self.comment.replace_range(at..at + ch.len_utf8(), "");
        self.comment_cursor = at;
    }

    pub fn comment_left(&mut self) {
        let at = self.cursor_at();
        if at == 0 {
            self.comment_cursor = 0;
            return;
        }
        self.comment_cursor = self.comment.floor_char_boundary(at - 1);
    }

    pub fn comment_right(&mut self) {
        let at = self.cursor_at();
        let Some(ch) = self.comment[at..].chars().next() else {
            self.comment_cursor = self.comment.len();
            return;
        };
        self.comment_cursor = at + ch.len_utf8();
    }

    pub fn comment_newline(&mut self) {
        self.insert_comment('\n');
    }

    pub fn clear_comment(&mut self) {
        self.comment.clear();
        self.comment_cursor = 0;
        self.comment_focused = false;
    }

    fn cursor_at(&self) -> usize {
        self.comment
            .floor_char_boundary(self.comment_cursor.min(self.comment.len()))
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

    pub fn scroll_max(&self, viewport: usize) -> usize {
        self.selected_file()
            .map(|file| file.visible_len())
            .unwrap_or(0)
            .saturating_sub(viewport)
    }

    pub fn scroll_by(&mut self, delta: isize, viewport: usize) {
        let max = self.scroll_max(viewport);
        let current = self.scroll.min(max);
        if delta < 0 {
            self.scroll = current.saturating_sub(delta.unsigned_abs());
        } else {
            self.scroll = current.saturating_add(delta as usize).min(max);
        }
    }

    pub fn scroll_bottom(&mut self, viewport: usize) {
        self.scroll = self.scroll_max(viewport);
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
        self.files.iter().fold((0, 0), |(adds, dels), file| {
            (adds + file.additions, dels + file.deletions)
        })
    }

    pub fn overlay_len(&self) -> usize {
        match self.overlay {
            Overlay::None | Overlay::Help => 0,
            Overlay::Commits => self.commits.len(),
            Overlay::Branches => self.branches.len(),
            Overlay::PullRequests => self.prs.len(),
            Overlay::Review => ReviewKind::CHOICES.len(),
            Overlay::Merge => 0,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{RepoContext, ViewState};

    fn view() -> ViewState {
        ViewState::new(RepoContext {
            root: PathBuf::from("."),
            branch: None,
            head: "abc".to_string(),
            default_branch: "main".to_string(),
        })
    }

    #[test]
    fn scrolling_up_from_the_bottom_moves_immediately() {
        use super::{ChangeKind, DiffLine, DiffLineKind, FileDiff, FileStatus};
        let mut view = view();
        view.files = vec![FileDiff {
            path: "a.txt".to_string(),
            old_path: None,
            status: FileStatus::Modified,
            kind: ChangeKind::Committed,
            additions: 10,
            deletions: 0,
            binary: false,
            lines: (0..10)
                .map(|index| DiffLine {
                    kind: DiffLineKind::Context,
                    old_lineno: Some(index),
                    new_lineno: Some(index),
                    text: format!("line {index}"),
                })
                .collect(),
        }];
        view.scroll = usize::MAX;
        view.scroll_by(-1, 3);
        assert_eq!(view.scroll, 6);
        view.scroll_by(100, 3);
        assert_eq!(view.scroll, 7);
    }

    #[test]
    fn comment_editing_keeps_the_cursor_on_character_boundaries() {
        let mut view = view();
        view.insert_comment('a');
        view.insert_comment('b');
        view.comment_left();
        view.comment_backspace();
        assert_eq!(view.comment, "b");
        view.comment_right();
        view.comment_newline();
        view.insert_comment('é');
        view.comment_backspace();
        assert_eq!(view.comment, "b\n");
        assert_eq!(view.comment_cursor, view.comment.len());
    }

    #[test]
    fn review_state_marks_unreviewed_until_a_review_exists() {
        use super::ReviewState;
        assert_eq!(
            ReviewState::from_reviews("", std::iter::empty()),
            ReviewState::Unreviewed
        );
        assert_eq!(
            ReviewState::from_reviews("REVIEW_REQUIRED", ["COMMENTED"]),
            ReviewState::Reviewed
        );
        assert_eq!(
            ReviewState::from_reviews("APPROVED", ["COMMENTED"]),
            ReviewState::Approved
        );
        assert_eq!(
            ReviewState::from_reviews("", ["CHANGES_REQUESTED"]),
            ReviewState::ChangesRequested
        );
    }

    #[test]
    fn merge_requires_passing_ci_and_permission() {
        use super::{CheckResult, MergeMethod, MergeReadiness};
        let passing = MergeReadiness::assess(
            "MERGEABLE",
            "CLEAN",
            false,
            &[("ci".to_string(), CheckResult::Pass)],
            Some(MergeMethod::Squash),
        );
        assert!(passing.can_merge());
        let failing = MergeReadiness::assess(
            "MERGEABLE",
            "UNSTABLE",
            false,
            &[("ci".to_string(), CheckResult::Fail)],
            Some(MergeMethod::Squash),
        );
        assert!(!failing.can_merge());
        assert!(!failing.ci_ok);
        assert!(failing.merge_ok);
        let blocked = MergeReadiness::assess(
            "MERGEABLE",
            "BLOCKED",
            false,
            &[("ci".to_string(), CheckResult::Pass)],
            Some(MergeMethod::Squash),
        );
        assert!(!blocked.can_merge());
        assert_eq!(blocked.merge_label, "blocked");
    }
}
