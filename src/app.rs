use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyboardEnhancementFlags, MouseEventKind,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;

use crate::gh;
use crate::git;
use crate::input::{self, Action};
use crate::model::{FileDiff, Overlay, RepoContext, ReviewTarget, ViewState};
use crate::ui;

const COMMIT_LIMIT: usize = 200;
const POLL: Duration = Duration::from_millis(100);
static TERMINAL_CLEANED: AtomicBool = AtomicBool::new(false);

pub fn run(cwd: PathBuf) -> Result<()> {
    let repo = git::discover(&cwd)?;
    let mut view = ViewState::new(repo);
    reload(&mut view);
    run_loop(&mut view)
}

fn run_loop(view: &mut ViewState) -> Result<()> {
    TERMINAL_CLEANED.store(false, Ordering::SeqCst);
    let mut terminal = ratatui::init();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup_terminal();
        previous(info);
    }));
    let _guard = TerminalGuard;
    execute!(stdout(), EnableMouseCapture)?;
    let _ = execute!(stdout(), PushKeyboardEnhancementFlags(keyboard_flags()));

    loop {
        terminal.draw(|frame| ui::render(frame, view))?;
        let Some(event) = poll_event()? else {
            continue;
        };
        match event {
            Event::Key(key) => {
                let action = input::action_for(key, view.overlay);
                if action == Action::Quit {
                    break;
                }
                apply_action(view, action, page_size(&terminal));
            }
            Event::Mouse(mouse) => apply_mouse(view, mouse.kind),
            _ => {}
        }
    }
    Ok(())
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        cleanup_terminal();
    }
}

fn cleanup_terminal() {
    if TERMINAL_CLEANED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = execute!(io::stdout(), DisableMouseCapture);
    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    ratatui::restore();
}

fn keyboard_flags() -> KeyboardEnhancementFlags {
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
}

fn poll_event() -> Result<Option<Event>> {
    match event::poll(POLL) {
        Ok(false) => Ok(None),
        Ok(true) => match event::read() {
            Ok(event) => Ok(Some(event)),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(None),
            Err(err) => Err(err.into()),
        },
        Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn page_size(terminal: &ratatui::DefaultTerminal) -> usize {
    let rows = terminal.size().map(|size| size.height).unwrap_or(0);
    usize::from(rows).saturating_sub(3).max(1)
}

fn apply_mouse(view: &mut ViewState, kind: MouseEventKind) {
    if view.overlay != Overlay::None {
        return;
    }
    match kind {
        MouseEventKind::ScrollUp => view.scroll_by(-3),
        MouseEventKind::ScrollDown => view.scroll_by(3),
        _ => {}
    }
}

fn apply_action(view: &mut ViewState, action: Action, page: usize) {
    let half = (page / 2).max(1);
    match action {
        Action::None | Action::Quit => {}
        Action::NextFile => view.next_file(),
        Action::PrevFile => view.prev_file(),
        Action::ScrollUp => view.scroll_by(-1),
        Action::ScrollDown => view.scroll_by(1),
        Action::PageUp => view.scroll_by(-(page as isize)),
        Action::PageDown => view.scroll_by(page as isize),
        Action::HalfPageUp => view.scroll_by(-(half as isize)),
        Action::HalfPageDown => view.scroll_by(half as isize),
        Action::ScrollTop => view.scroll = 0,
        Action::ScrollBottom => view.scroll = usize::MAX,
        Action::OpenCommits => open_commits(view),
        Action::OpenBranches => open_branches(view),
        Action::OpenPullRequests => open_pull_requests(view),
        Action::WorkingTree => show_working_tree(view),
        Action::Refresh => reload(view),
        Action::ToggleHelp => toggle_help(view),
        Action::OverlayUp => view.overlay_prev(),
        Action::OverlayDown => view.overlay_next(),
        Action::Confirm => confirm(view),
        Action::Cancel => view.close_overlay(),
        Action::MarkBase => mark_base(view),
    }
}

fn toggle_help(view: &mut ViewState) {
    if view.overlay == Overlay::Help {
        view.close_overlay();
    } else {
        view.open_overlay(Overlay::Help);
    }
}

fn show_working_tree(view: &mut ViewState) {
    view.target = ReviewTarget::WorkingTree;
    view.pending_base = None;
    view.close_overlay();
    reload(view);
}

fn open_commits(view: &mut ViewState) {
    match git::list_commits(&view.repo, COMMIT_LIMIT) {
        Ok(commits) => view.commits = commits,
        Err(err) => view.status = error_status(err),
    }
    view.open_overlay(Overlay::Commits);
}

fn open_branches(view: &mut ViewState) {
    match git::list_branches(&view.repo) {
        Ok(branches) => view.branches = branches,
        Err(err) => view.status = error_status(err),
    }
    view.open_overlay(Overlay::Branches);
}

fn open_pull_requests(view: &mut ViewState) {
    match gh::list_prs(&view.repo) {
        Ok(prs) => view.prs = prs,
        Err(err) => view.status = error_status(err),
    }
    view.open_overlay(Overlay::PullRequests);
}

fn confirm(view: &mut ViewState) {
    match view.overlay {
        Overlay::Commits => confirm_commit(view),
        Overlay::Branches => confirm_branch(view),
        Overlay::PullRequests => confirm_pull_request(view),
        Overlay::Help | Overlay::None => {}
    }
}

fn confirm_commit(view: &mut ViewState) {
    let Some(selected) = view.commits.get(view.overlay_selected).cloned() else {
        return;
    };
    let target = match view.pending_base.clone() {
        Some(base) if base != selected.sha => ReviewTarget::Range {
            base,
            head: selected.sha,
        },
        _ => ReviewTarget::Commit { rev: selected.sha },
    };
    view.pending_base = None;
    view.target = target;
    view.close_overlay();
    reload(view);
}

fn confirm_branch(view: &mut ViewState) {
    let Some(branch) = view.branches.get(view.overlay_selected).cloned() else {
        return;
    };
    if branch.name == view.repo.default_branch {
        view.status = format!("error: {} is the default branch", branch.name);
        return;
    }
    view.target = ReviewTarget::Range {
        base: view.repo.default_branch.clone(),
        head: branch.name,
    };
    view.close_overlay();
    reload(view);
}

fn confirm_pull_request(view: &mut ViewState) {
    let Some(pr) = view.prs.get(view.overlay_selected).cloned() else {
        return;
    };
    let target = ReviewTarget::PullRequest {
        number: pr.number,
        title: pr.title,
        base_ref: pr.base_ref,
        head_ref: pr.head_ref,
    };
    match load(&view.repo, &target) {
        Ok(files) => {
            view.target = target;
            view.set_files(files);
            view.status = pr.url;
            view.close_overlay();
        }
        Err(err) => view.status = error_status(err),
    }
}

fn mark_base(view: &mut ViewState) {
    if view.overlay != Overlay::Commits {
        return;
    }
    let Some(commit) = view.commits.get(view.overlay_selected).cloned() else {
        return;
    };
    if view.pending_base.as_deref() == Some(commit.sha.as_str()) {
        view.pending_base = None;
        view.status = "base cleared".to_string();
    } else {
        view.status = format!("base {} — enter a head commit", commit.short_sha);
        view.pending_base = Some(commit.sha);
    }
}

fn reload(view: &mut ViewState) {
    let target = view.target.clone();
    match load(&view.repo, &target) {
        Ok(files) => {
            view.status = loaded_status(&target, &files);
            view.set_files(files);
        }
        Err(err) => view.status = error_status(err),
    }
}

fn load(repo: &RepoContext, target: &ReviewTarget) -> Result<Vec<FileDiff>> {
    match target {
        ReviewTarget::WorkingTree => git::working_tree(repo),
        ReviewTarget::Commit { rev } => git::commit_diff(repo, rev),
        ReviewTarget::Range { base, head } => git::range_diff(repo, base, head),
        ReviewTarget::PullRequest { number, .. } => {
            let diff = gh::pr_diff(repo, *number)?;
            Ok(git::parse_unified_diff(&diff))
        }
    }
}

fn loaded_status(target: &ReviewTarget, files: &[FileDiff]) -> String {
    if files.is_empty() {
        if matches!(target, ReviewTarget::WorkingTree) {
            "working tree clean".to_string()
        } else {
            "no changes".to_string()
        }
    } else {
        let adds: u32 = files.iter().map(|file| file.additions).sum();
        let dels: u32 = files.iter().map(|file| file.deletions).sum();
        format!("{} files  +{adds} -{dels}", files.len())
    }
}

fn error_status(err: impl std::fmt::Display) -> String {
    format!("error: {err}")
}
