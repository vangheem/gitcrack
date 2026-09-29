use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::ViewState;

use super::text::{draw, fill, style, truncate, width};
use super::theme::{FG, GRAY, PANEL, RED};

const HINTS: &str = "shift-up/down file  up/down scroll  c commits  b branch  p prs  w tree  r refresh  ? help  q quit";

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) {
    fill(frame, area, style(FG, PANEL));
    if area.width < 2 {
        return;
    }
    let usable = (area.width - 2) as usize;
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
    let hints = truncate(HINTS, usable.saturating_sub(status_w + gap));
    draw(
        frame,
        area.x + 1,
        area.y,
        &hints,
        usable,
        style(GRAY, PANEL),
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
