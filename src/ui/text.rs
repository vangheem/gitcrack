use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::model::FileStatus;

use super::theme::{self, BORDER, GRAY, GREEN, RED};

pub fn width(text: &str) -> usize {
    text.width()
}

pub fn style(fg: Color, bg: Color) -> Style {
    Style::new().fg(fg).bg(bg)
}

pub fn dim(fg: Color, bg: Color) -> Style {
    style(fg, bg).add_modifier(ratatui::style::Modifier::DIM)
}

pub fn fill(frame: &mut Frame, area: Rect, paint: Style) {
    if area.is_empty() {
        return;
    }
    frame.buffer_mut().set_style(area, paint);
}

pub fn draw(frame: &mut Frame, x: u16, y: u16, text: &str, max_width: usize, paint: Style) {
    if max_width == 0 || text.is_empty() {
        return;
    }
    frame.buffer_mut().set_stringn(x, y, text, max_width, paint);
}

pub fn truncate(text: &str, max: usize) -> String {
    if width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    const ELLIPSIS: &str = "…";
    let ellipsis_w = width(ELLIPSIS);
    if max <= ellipsis_w {
        return ELLIPSIS.to_string();
    }
    let budget = max - ellipsis_w;
    let mut used = 0;
    let mut end = 0;
    for (idx, ch) in text.char_indices() {
        let ch_w = ch.width().unwrap_or(0);
        if used + ch_w > budget {
            break;
        }
        used += ch_w;
        end = idx + ch.len_utf8();
    }
    let mut out = text[..end].to_string();
    out.push('…');
    out
}

pub fn visible(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\t' {
            out.push_str("    ");
        } else if !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

pub fn window_start(selected: usize, len: usize, visible: usize) -> usize {
    if visible == 0 || len <= visible {
        return 0;
    }
    let selected = selected.min(len - 1);
    selected.saturating_sub(visible / 2).min(len - visible)
}

pub fn clamp_scroll(scroll: usize, len: usize, viewport: usize) -> usize {
    scroll.min(len.saturating_sub(viewport))
}

pub fn status_fg(status: FileStatus) -> Color {
    match status {
        FileStatus::Added | FileStatus::Copied => GREEN,
        FileStatus::Modified | FileStatus::Typechange => theme::YELLOW,
        FileStatus::Deleted => RED,
        FileStatus::Renamed => theme::CYAN,
        FileStatus::Untracked => GRAY,
    }
}

pub fn stats_width(adds: u32, dels: u32) -> usize {
    format!("+{adds} -{dels}").width()
}

pub fn draw_stats(frame: &mut Frame, right: u16, y: u16, adds: u32, dels: u32, bg: Color) {
    let adds_s = format!("+{adds}");
    let dels_s = format!("-{dels}");
    let total = adds_s.width() + 1 + dels_s.width();
    let x = right.saturating_sub(total as u16);
    draw(frame, x, y, &adds_s, adds_s.width(), style(GREEN, bg));
    draw(
        frame,
        x + adds_s.width() as u16 + 1,
        y,
        &dels_s,
        dels_s.width(),
        style(RED, bg),
    );
}

pub fn centered(area: Rect, width_pct: u32, height_pct: u32) -> Rect {
    if area.width == 0 || area.height == 0 {
        return area;
    }
    let width = (u32::from(area.width) * width_pct / 100).clamp(1, u32::from(area.width)) as u16;
    let height =
        (u32::from(area.height) * height_pct / 100).clamp(1, u32::from(area.height)) as u16;
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub fn scrollbar(
    frame: &mut Frame,
    area: Rect,
    len: usize,
    position: usize,
    viewport: usize,
    bg: Color,
) {
    if len <= viewport || area.width == 0 || area.height == 0 {
        return;
    }
    let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .thumb_style(style(GRAY, bg))
        .track_style(style(BORDER, bg));
    let mut state = ScrollbarState::new(len)
        .position(position)
        .viewport_content_length(viewport);
    frame.render_stateful_widget(bar, area, &mut state);
}
