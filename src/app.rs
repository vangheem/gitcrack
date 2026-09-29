use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyboardEnhancementFlags, MouseButton,
    MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;

use crate::clipboard;
use crate::gh;
use crate::git;
use crate::input::{self, Action};
use crate::model::{FileDiff, Overlay, RepoContext, ReviewTarget, ViewState};
use crate::select::{Screen, Selection};
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
    let mut screen = Screen::default();
    let mut selection = None;
    let mut dragging = false;

    loop {
        let shown = selection.filter(|sel: &Selection| !sel.is_empty());
        terminal.draw(|frame| {
            ui::render(frame, view);
            if let Some(sel) = shown {
                ui::paint_selection(frame, sel);
            }
            screen = Screen::from_buffer(frame.buffer_mut());
        })?;
        let Some(event) = poll_event()? else {
            continue;
        };
        match event {
            Event::Key(key) => {
                selection = None;
                dragging = false;
                let action = input::action_for(key, view.overlay, view.comment_focused);
                if action == Action::Quit {
                    break;
                }
                apply_action(view, action, page_size(&terminal, view));
            }
            Event::Mouse(mouse) => {
                apply_mouse(view, mouse, &screen, &mut selection, &mut dragging);
            }
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

fn page_size(terminal: &ratatui::DefaultTerminal, view: &ViewState) -> usize {
    let rows = terminal.size().map(|size| size.height).unwrap_or(0);
    let chrome = 3 + usize::from(view.comment_rows());
    usize::from(rows).saturating_sub(chrome).max(1)
}

fn apply_mouse(
    view: &mut ViewState,
    mouse: MouseEvent,
    screen: &Screen,
    selection: &mut Option<Selection>,
    dragging: &mut bool,
) {
    let pos = screen.clamp(mouse.column, mouse.row);
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *selection = Some(Selection {
                anchor: pos,
                head: pos,
            });
            *dragging = true;
        }
        MouseEventKind::Drag(_) if *dragging => {
            if let Some(sel) = selection.as_mut() {
                sel.head = pos;
            }
        }
        MouseEventKind::Up(_) if *dragging => {
            *dragging = false;
            finish_selection(view, screen, selection, pos);
        }
        MouseEventKind::ScrollUp => wheel(view, selection, dragging, -3),
        MouseEventKind::ScrollDown => wheel(view, selection, dragging, 3),
        _ => {}
    }
}

fn wheel(
    view: &mut ViewState,
    selection: &mut Option<Selection>,
    dragging: &mut bool,
    delta: isize,
) {
    *selection = None;
    *dragging = false;
    if view.overlay == Overlay::None {
        view.scroll_by(delta);
    }
}

fn finish_selection(
    view: &mut ViewState,
    screen: &Screen,
    selection: &mut Option<Selection>,
    pos: (u16, u16),
) {
    let Some(mut sel) = selection.take() else {
        return;
    };
    sel.head = pos;
    if sel.is_empty() {
        return;
    }
    let text = screen.selected_text(sel);
    if text.is_empty() {
        return;
    }
    view.status = if clipboard::copy(&text) {
        "copied".to_string()
    } else {
        "copy failed".to_string()
    };
    *selection = Some(sel);
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
        Action::FocusComment => focus_comment(view),
        Action::OpenReview => open_review(view),
        Action::BlurComment => view.comment_focused = false,
        Action::InsertChar(ch) => view.insert_comment(ch),
        Action::CommentBackspace => view.comment_backspace(),
        Action::CommentDelete => view.comment_delete(),
        Action::CommentLeft => view.comment_left(),
        Action::CommentRight => view.comment_right(),
        Action::CommentNewline => view.comment_newline(),
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
    replace_target(view, ReviewTarget::WorkingTree);
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
        Overlay::Review => submit_review(view),
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
    replace_target(view, target);
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
    replace_target(
        view,
        ReviewTarget::Range {
            base: view.repo.default_branch.clone(),
            head: branch.name,
        },
    );
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
            replace_target(view, target);
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

fn focus_comment(view: &mut ViewState) {
    if !view.is_pull_request() {
        view.status = "error: open a pull request to comment".to_string();
        return;
    }
    view.comment_focused = true;
    view.comment_cursor = view.comment_cursor.min(view.comment.len());
}

fn open_review(view: &mut ViewState) {
    if view.target.pr_number().is_none() {
        view.status = "error: open a pull request to review".to_string();
        return;
    }
    view.open_overlay(Overlay::Review);
}

fn submit_review(view: &mut ViewState) {
    let Some(number) = view.target.pr_number() else {
        view.status = "error: open a pull request to review".to_string();
        return;
    };
    let kind = view.review_choice();
    let body = view.comment.trim().to_string();
    if body.is_empty() && kind.needs_body() {
        view.status = "error: a comment is required".to_string();
        return;
    }
    match gh::review_pr(&view.repo, number, kind, &body) {
        Ok(()) => {
            view.clear_comment();
            view.close_overlay();
            view.status = kind.submitted().to_string();
        }
        Err(err) => view.status = error_status(err),
    }
}

fn replace_target(view: &mut ViewState, target: ReviewTarget) {
    if view.target.pr_number() != target.pr_number() {
        view.clear_comment();
    }
    view.target = target;
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
