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
}

pub fn action_for(key: KeyEvent, overlay: Overlay) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
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
    match key.code {
        KeyCode::Up if shift => Action::PrevFile,
        KeyCode::Down if shift => Action::NextFile,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => Action::ScrollUp,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => Action::ScrollDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::ScrollTop,
        KeyCode::End => Action::ScrollBottom,
        KeyCode::Char('[') => Action::PrevFile,
        KeyCode::Char(']') => Action::NextFile,
        KeyCode::Char('c') | KeyCode::Char('C') => Action::OpenCommits,
        KeyCode::Char('b') | KeyCode::Char('B') => Action::OpenBranches,
        KeyCode::Char('p') | KeyCode::Char('P') => Action::OpenPullRequests,
        KeyCode::Char('w') | KeyCode::Char('W') => Action::WorkingTree,
        KeyCode::Char('r') | KeyCode::Char('R') => Action::Refresh,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Action::Quit,
        _ => Action::None,
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
    fn shift_arrows_change_file_and_arrows_scroll() {
        assert_eq!(
            action_for(key(KeyCode::Down, KeyModifiers::SHIFT), Overlay::None),
            Action::NextFile
        );
        assert_eq!(
            action_for(key(KeyCode::Up, KeyModifiers::SHIFT), Overlay::None),
            Action::PrevFile
        );
        assert_eq!(
            action_for(key(KeyCode::Down, KeyModifiers::empty()), Overlay::None),
            Action::ScrollDown
        );
        assert_eq!(
            action_for(key(KeyCode::Up, KeyModifiers::empty()), Overlay::None),
            Action::ScrollUp
        );
    }

    #[test]
    fn overlay_enter_confirms_and_escape_cancels() {
        assert_eq!(
            action_for(key(KeyCode::Enter, KeyModifiers::empty()), Overlay::Commits),
            Action::Confirm
        );
        assert_eq!(
            action_for(key(KeyCode::Esc, KeyModifiers::empty()), Overlay::Commits),
            Action::Cancel
        );
        assert_eq!(
            action_for(
                key(KeyCode::Char(' '), KeyModifiers::empty()),
                Overlay::Commits
            ),
            Action::MarkBase
        );
    }

    #[test]
    fn release_events_are_ignored() {
        let event = KeyEvent::new_with_kind(
            KeyCode::Down,
            KeyModifiers::empty(),
            KeyEventKind::Release,
        );
        assert_eq!(action_for(event, Overlay::None), Action::None);
    }
}
