use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::model::Overlay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    NextFile,
    PrevFile,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    ScrollTop,
    ScrollBottom,
    OpenCommits,
    OpenBranches,
    OpenPullRequests,
    WorkingTree,
    Refresh,
    ToggleHelp,
    OverlayUp,
    OverlayDown,
    Confirm,
    Cancel,
    MarkBase,
    FocusComment,
    OpenReview,
    OpenMerge,
    BlurComment,
    InsertChar(char),
    CommentBackspace,
    CommentDelete,
    CommentLeft,
    CommentRight,
    CommentNewline,
}

pub fn action_for(key: KeyEvent, overlay: Overlay, comment_focused: bool) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    if overlay == Overlay::Review || overlay == Overlay::Merge {
        return review_modal_action(key);
    }
    if overlay == Overlay::None && comment_focused {
        return comment_action(key);
    }
    if overlay == Overlay::None {
        review_action(key)
    } else {
        overlay_action(key)
    }
}

fn review_action(key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    if ctrl || alt {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => Action::HalfPageUp,
            KeyCode::Char('d') | KeyCode::Char('D') => Action::HalfPageDown,
            _ => Action::None,
        };
    }
    if shifted(&key, 'c') {
        return Action::FocusComment;
    }
    if shifted(&key, 'r') {
        return Action::OpenReview;
    }
    if shifted(&key, 'm') {
        return Action::OpenMerge;
    }
    match key.code {
        KeyCode::Up if shift => Action::ScrollUp,
        KeyCode::Down if shift => Action::ScrollDown,
        KeyCode::Up => Action::PrevFile,
        KeyCode::Down => Action::NextFile,
        KeyCode::Char('k') | KeyCode::Char('K') => Action::ScrollUp,
        KeyCode::Char('j') | KeyCode::Char('J') => Action::ScrollDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown | KeyCode::Char(' ') => Action::PageDown,
        KeyCode::Home => Action::ScrollTop,
        KeyCode::End => Action::ScrollBottom,
        KeyCode::BackTab => Action::PrevFile,
        KeyCode::Tab if shift => Action::PrevFile,
        KeyCode::Tab => Action::NextFile,
        KeyCode::Char('[') => Action::PrevFile,
        KeyCode::Char(']') => Action::NextFile,
        KeyCode::Char('c') => Action::OpenCommits,
        KeyCode::Char('b') | KeyCode::Char('B') => Action::OpenBranches,
        KeyCode::Char('p') | KeyCode::Char('P') => Action::OpenPullRequests,
        KeyCode::Char('w') | KeyCode::Char('W') => Action::WorkingTree,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Action::Quit,
        _ => Action::None,
    }
}

fn comment_action(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return Action::None;
    }
    if shifted(&key, 'r') {
        return Action::OpenReview;
    }
    if shifted(&key, 'm') {
        return Action::OpenMerge;
    }
    if shifted(&key, 'c') {
        return Action::None;
    }
    match key.code {
        KeyCode::Esc => Action::BlurComment,
        KeyCode::Backspace => Action::CommentBackspace,
        KeyCode::Delete => Action::CommentDelete,
        KeyCode::Left => Action::CommentLeft,
        KeyCode::Right => Action::CommentRight,
        KeyCode::Enter => Action::CommentNewline,
        KeyCode::Char(ch) => Action::InsertChar(typed_char(ch, &key)),
        _ => Action::None,
    }
}

fn review_modal_action(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return Action::None;
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => Action::OverlayUp,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => Action::OverlayDown,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => Action::Cancel,
        _ => Action::None,
    }
}

fn shifted(key: &KeyEvent, letter: char) -> bool {
    let KeyCode::Char(ch) = key.code else {
        return false;
    };
    ch.eq_ignore_ascii_case(&letter)
        && (key.modifiers.contains(KeyModifiers::SHIFT) || ch.is_ascii_uppercase())
}

fn typed_char(ch: char, key: &KeyEvent) -> char {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        ch.to_ascii_uppercase()
    } else {
        ch
    }
}

fn overlay_action(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return Action::None;
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => Action::OverlayUp,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => Action::OverlayDown,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => Action::Cancel,
        KeyCode::Char(' ') => Action::MarkBase,
        KeyCode::Char('c') | KeyCode::Char('C') => Action::OpenCommits,
        KeyCode::Char('b') | KeyCode::Char('B') => Action::OpenBranches,
        KeyCode::Char('p') | KeyCode::Char('P') => Action::OpenPullRequests,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('w') | KeyCode::Char('W') => Action::WorkingTree,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn arrows_change_file_and_shift_or_jk_scroll() {
        assert_eq!(
            action_for(
                key(KeyCode::Down, KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::NextFile
        );
        assert_eq!(
            action_for(
                key(KeyCode::Up, KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::PrevFile
        );
        assert_eq!(
            action_for(
                key(KeyCode::Down, KeyModifiers::SHIFT),
                Overlay::None,
                false
            ),
            Action::ScrollDown
        );
        assert_eq!(
            action_for(key(KeyCode::Up, KeyModifiers::SHIFT), Overlay::None, false),
            Action::ScrollUp
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('j'), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::ScrollDown
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('k'), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::ScrollUp
        );
        assert_eq!(
            action_for(
                key(KeyCode::Tab, KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::NextFile
        );
        assert_eq!(
            action_for(
                key(KeyCode::BackTab, KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::PrevFile
        );
        assert_eq!(
            action_for(key(KeyCode::Tab, KeyModifiers::SHIFT), Overlay::None, false),
            Action::PrevFile
        );
    }

    #[test]
    fn overlay_enter_confirms_and_escape_cancels() {
        assert_eq!(
            action_for(
                key(KeyCode::Enter, KeyModifiers::empty()),
                Overlay::Commits,
                false
            ),
            Action::Confirm
        );
        assert_eq!(
            action_for(
                key(KeyCode::Esc, KeyModifiers::empty()),
                Overlay::Commits,
                false
            ),
            Action::Cancel
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char(' '), KeyModifiers::empty()),
                Overlay::Commits,
                false
            ),
            Action::MarkBase
        );
    }

    #[test]
    fn release_events_are_ignored() {
        let event =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::empty(), KeyEventKind::Release);
        assert_eq!(action_for(event, Overlay::None, false), Action::None);
    }

    #[test]
    fn space_pages_down_unless_picking_a_commit_base() {
        assert_eq!(
            action_for(
                key(KeyCode::Char(' '), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::PageDown
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char(' '), KeyModifiers::empty()),
                Overlay::Commits,
                false
            ),
            Action::MarkBase
        );
    }

    #[test]
    fn shift_c_focuses_comment_and_shift_r_opens_review() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('C'), KeyModifiers::SHIFT),
                Overlay::None,
                false
            ),
            Action::FocusComment
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('c'), KeyModifiers::SHIFT),
                Overlay::None,
                false
            ),
            Action::FocusComment
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('R'), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::OpenReview
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('c'), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::OpenCommits
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('r'), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::Refresh
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('M'), KeyModifiers::empty()),
                Overlay::None,
                false
            ),
            Action::OpenMerge
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('m'), KeyModifiers::SHIFT),
                Overlay::None,
                true
            ),
            Action::OpenMerge
        );
    }

    #[test]
    fn focused_comment_types_and_shift_r_still_reviews() {
        assert_eq!(
            action_for(
                key(KeyCode::Char('a'), KeyModifiers::empty()),
                Overlay::None,
                true
            ),
            Action::InsertChar('a')
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char('r'), KeyModifiers::SHIFT),
                Overlay::None,
                true
            ),
            Action::OpenReview
        );
        assert_eq!(
            action_for(
                key(KeyCode::Esc, KeyModifiers::empty()),
                Overlay::None,
                true
            ),
            Action::BlurComment
        );
        assert_eq!(
            action_for(
                key(KeyCode::Enter, KeyModifiers::empty()),
                Overlay::Review,
                true
            ),
            Action::Confirm
        );
    }
}
