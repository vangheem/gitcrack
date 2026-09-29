use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::model::ViewState;

use super::text::{draw, fill, style, truncate, width};
use super::theme::{FG, GRAY, PANEL, RED};

const HINTS: &[&str] = &[
    "up/down tab file",
    "shift-up/down j/k scroll",
    "space page",
    "c commits",
    "b branch",
    "p prs",
    "w tree",
    "r refresh",
    "? help",
    "q quit",
];
const SEP: Color = Color::Rgb(100, 108, 118);

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) {
    fill(frame, area, style(FG, PANEL));
    if area.width < 2 {
        return;
    }
    let usable = (area.width - 2) as usize;
    let hints = hint_list(view);
    let status = view.status.as_str();
    let status_fg = if status.starts_with("error:") {
        RED
    } else {
        FG
    };
    let status_text = truncate(status, width(status).min(usable));
    let status_w = width(&status_text);
    let gap = if status_w == 0 {
        0
    } else {
        2.min(usable.saturating_sub(status_w))
    };
    draw_hints(
        frame,
        area.x + 1,
        area.y,
        usable.saturating_sub(status_w + gap),
        &hints,
    );
    if status_w > 0 {
        let x = area.right() - 1 - status_w as u16;
        draw(
            frame,
            x,
            area.y,
            &status_text,
            status_w,
            style(status_fg, PANEL),
        );
    }
}

fn hint_list(view: &ViewState) -> Vec<&str> {
    if view.comment_focused {
        return vec!["esc done", "enter newline", "shift-r review"];
    }
    let mut hints = Vec::new();
    if view.is_pull_request() {
        hints.extend(["shift-c comment", "shift-r review"]);
    }
    hints.extend(HINTS);
    hints
}

fn draw_hints(frame: &mut Frame, mut x: u16, y: u16, max: usize, hints: &[&str]) {
    let mut used = 0;
    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            if used + 3 > max {
                break;
            }
            draw(frame, x + 1, y, "|", 1, style(SEP, PANEL));
            used += 3;
            x += 3;
        }
        let room = max - used;
        if room == 0 {
            break;
        }
        let full = width(hint);
        let text = if full <= room {
            (*hint).to_string()
        } else {
            truncate(hint, room)
        };
        let drawn = width(&text);
        draw(frame, x, y, &text, room, style(GRAY, PANEL));
        used += drawn;
        x += drawn as u16;
        if drawn < full {
            break;
        }
    }
}
