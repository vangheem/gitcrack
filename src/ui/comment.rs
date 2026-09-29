use ratatui::Frame;
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthChar;

use crate::model::{ReviewState, ViewState};

use super::text::{draw, fill, style, truncate, width};
use super::theme::{ACCENT, FG, GRAY, PANEL};

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) {
    if area.is_empty() || !view.is_pull_request() {
        return;
    }
    fill(frame, area, style(FG, PANEL));
    if area.width < 4 {
        return;
    }
    let x = area.x + 1;
    let text_x = x + 2;
    let text_w = (area.width as usize).saturating_sub(4);
    let prompt = if view.comment_focused { ACCENT } else { GRAY };
    draw(frame, x, area.y, ">", 1, style(prompt, PANEL));
    if !view.comment_focused && view.comment.is_empty() {
        draw(
            frame,
            text_x,
            area.y,
            "shift-c comment",
            text_w,
            style(GRAY, PANEL),
        );
        return;
    }
    let hint = if view.comment_focused {
        ""
    } else if view.pr_review == Some(ReviewState::Approved) {
        "shift-m"
    } else {
        "shift-r"
    };
    let hint_w = width(hint);
    let reserve = if !hint.is_empty() && text_w > hint_w + 4 {
        hint_w + 2
    } else {
        0
    };
    let line_w = text_w.saturating_sub(reserve);
    if reserve > 0 {
        let hint_x = text_x + line_w as u16 + 2;
        draw(frame, hint_x, area.y, hint, hint_w, style(GRAY, PANEL));
    }
    let lines = comment_lines(&view.comment);
    let (line_idx, col) = cursor_in_line(&view.comment, view.comment_cursor);
    let height = area.height as usize;
    let start = line_idx.saturating_add(1).saturating_sub(height);
    let rows = height.min(lines.len().saturating_sub(start));
    for row in 0..rows {
        let index = start + row;
        let y = area.y + row as u16;
        let cursor = if view.comment_focused && index == line_idx {
            Some(col)
        } else {
            None
        };
        draw_line(frame, text_x, y, lines[index], cursor, line_w);
    }
}

fn comment_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        vec![""]
    } else {
        text.split('\n').collect()
    }
}

fn cursor_in_line(text: &str, cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(text.len());
    let cursor = text.floor_char_boundary(cursor);
    let before = &text[..cursor];
    let line = before.matches('\n').count();
    let col = before
        .rsplit_once('\n')
        .map(|(_, rest)| rest.len())
        .unwrap_or(before.len());
    (line, col)
}

fn draw_line(frame: &mut Frame, x: u16, y: u16, line: &str, cursor: Option<usize>, max: usize) {
    if max == 0 {
        return;
    }
    let (shown, cursor_col) = visible_line(line, cursor, max);
    draw(frame, x, y, &shown, max, style(FG, PANEL));
    let Some(col) = cursor_col else {
        return;
    };
    if col >= max {
        return;
    }
    let cursor_x = x + col as u16;
    let glyph = cursor_glyph(&shown, col);
    draw(
        frame,
        cursor_x,
        y,
        &glyph,
        width(&glyph).max(1),
        style(PANEL, ACCENT),
    );
}

fn cursor_glyph(shown: &str, col: usize) -> String {
    let rest = skip_cols(shown, col);
    rest.chars()
        .next()
        .map(|ch| ch.to_string())
        .unwrap_or_else(|| " ".to_string())
}

fn visible_line(line: &str, cursor: Option<usize>, max: usize) -> (String, Option<usize>) {
    let cursor_col = cursor.map(|at| {
        let at = line.floor_char_boundary(at.min(line.len()));
        width(&line[..at])
    });
    let skip = cursor_col
        .map(|col| col.saturating_sub(max.saturating_sub(1)))
        .unwrap_or(0);
    let start = skip_cols(line, skip);
    let shown = if cursor.is_some() {
        take_cols(start, max)
    } else {
        truncate(start, max)
    };
    let cursor_at = cursor_col.map(|col| col.saturating_sub(skip_width(line, start)));
    (shown, cursor_at)
}

fn take_cols(text: &str, max: usize) -> String {
    let mut used = 0;
    let mut end = 0;
    for (idx, ch) in text.char_indices() {
        let ch_w = ch.width().unwrap_or(0);
        if used + ch_w > max {
            break;
        }
        used += ch_w;
        end = idx + ch.len_utf8();
    }
    text[..end].to_string()
}

fn skip_cols(text: &str, cols: usize) -> &str {
    if cols == 0 {
        return text;
    }
    let mut used = 0;
    for (idx, ch) in text.char_indices() {
        if used >= cols {
            return &text[idx..];
        }
        used += ch.width().unwrap_or(0);
    }
    ""
}

fn skip_width(line: &str, rest: &str) -> usize {
    let skipped = line.len().saturating_sub(rest.len());
    let skipped = line.floor_char_boundary(skipped.min(line.len()));
    width(&line[..skipped])
}
