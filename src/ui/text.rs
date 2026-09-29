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

pub fn fit_path(path: &str, max: usize) -> String {
    if width(path) <= max {
        return path.to_string();
    }
    if let Some((old, new)) = path.split_once(" -> ") {
        return fit_rename(old, new, max);
    }
    fit_segments(path, max)
}

fn fit_segments(path: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let Some((parent, file)) = path.rsplit_once('/') else {
        return fit_leaf(path, max);
    };
    if parent.is_empty() || file.is_empty() {
        return fit_leaf(file, max);
    }
    let file_w = width(file);
    if file_w >= max {
        return fit_leaf(file, max);
    }
    let parent_budget = max - file_w - 1;
    match compress_dirs(parent, parent_budget) {
        Some(shown) => format!("{shown}/{file}"),
        None => file.to_string(),
    }
}

fn compress_dirs(parent: &str, budget: usize) -> Option<String> {
    if width(parent) <= budget {
        return Some(parent.to_string());
    }
    const DOTS: &str = "...";
    let dots_w = width(DOTS);
    let parts: Vec<&str> = parent.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() || budget < dots_w {
        return None;
    }
    let bridge = dots_w + 1;
    if budget >= bridge {
        let head_budget = budget - bridge;
        if let Some(head) = leading_dirs(&parts, head_budget) {
            return Some(format!("{head}/{DOTS}"));
        }
        let stub = take_prefix(parts[0], head_budget);
        if !stub.is_empty() {
            return Some(format!("{stub}{DOTS}"));
        }
    }
    Some(DOTS.to_string())
}

fn leading_dirs(parts: &[&str], budget: usize) -> Option<String> {
    if budget == 0 {
        return None;
    }
    let mut used = 0;
    let mut count = 0;
    for (i, part) in parts.iter().enumerate() {
        let extra = width(part) + usize::from(i > 0);
        if used + extra > budget {
            break;
        }
        used += extra;
        count = i + 1;
    }
    if count == 0 {
        None
    } else {
        Some(parts[..count].join("/"))
    }
}

fn fit_leaf(name: &str, max: usize) -> String {
    if width(name) <= max {
        return name.to_string();
    }
    if max == 0 {
        return String::new();
    }
    const DOTS: &str = "...";
    let dots_w = width(DOTS);
    if max <= dots_w {
        return take_suffix(name, max);
    }
    format!("{DOTS}{}", take_suffix(name, max - dots_w))
}

fn fit_rename(old: &str, new: &str, max: usize) -> String {
    const ARROW: &str = " -> ";
    let arrow_w = width(ARROW);
    if max <= arrow_w {
        return fit_segments(new, max);
    }
    let avail = max - arrow_w;
    let new_w = width(new);
    if new_w <= avail {
        let old_budget = avail - new_w;
        if old_budget == 0 {
            return new.to_string();
        }
        let old_shown = fit_segments(old, old_budget);
        if old_shown.is_empty() {
            return new.to_string();
        }
        return format!("{old_shown}{ARROW}{new}");
    }
    let file_w = width(file_name(new)).min(avail);
    let new_budget = file_w.max(avail / 2).min(avail);
    let old_budget = avail - new_budget;
    let new_shown = fit_segments(new, new_budget);
    let slack = new_budget.saturating_sub(width(&new_shown));
    let old_shown = fit_segments(old, old_budget + slack);
    if old_shown.is_empty() {
        return new_shown;
    }
    let combined = format!("{old_shown}{ARROW}{new_shown}");
    if width(&combined) <= max {
        combined
    } else {
        new_shown
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map(|(_, file)| file).unwrap_or(path)
}

fn take_prefix(text: &str, max: usize) -> String {
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

fn take_suffix(text: &str, max: usize) -> String {
    let mut used = 0;
    let mut start = text.len();
    for (idx, ch) in text.char_indices().rev() {
        let ch_w = ch.width().unwrap_or(0);
        if used + ch_w > max {
            break;
        }
        used += ch_w;
        start = idx;
    }
    text[start..].to_string()
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

pub fn paint_selection(frame: &mut Frame, selection: crate::select::Selection) {
    let buffer = frame.buffer_mut();
    let area = buffer.area;
    if area.height == 0 || area.width == 0 {
        return;
    }
    let (start, end) = selection.ordered();
    let y0 = start.1.max(area.y);
    let y1 = end.1.min(area.bottom().saturating_sub(1));
    if y0 > y1 {
        return;
    }
    let right = area.right().saturating_sub(1);
    for y in y0..=y1 {
        let Some((x0, x1)) = selection.col_bounds(y) else {
            continue;
        };
        let x0 = x0.max(area.x);
        let x1 = x1.min(right);
        if x0 > x1 {
            continue;
        }
        for x in x0..=x1 {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_style(style(super::theme::FG, super::theme::SELECTED));
            }
        }
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

#[cfg(test)]
mod tests {
    use super::{fit_path, width};

    #[test]
    fn fit_path_keeps_filename_then_start() {
        let path = "src/ui/components/widgets/button.rs";
        let shown = fit_path(path, 22);
        assert_eq!(shown, "src/ui/.../button.rs");
        assert!(shown.ends_with("button.rs"));
        assert!(shown.starts_with("src"));
        assert_eq!(fit_path("a/b/c/d/e/file.rs", 15), "a/b/.../file.rs");
        assert_eq!(fit_path(path, 80), path);
    }

    #[test]
    fn fit_path_stays_within_budget() {
        let paths = [
            "src/ui/components/widgets/button.rs",
            "a/b/c/d/e/file.rs",
            "verylongdirectory/file.rs",
            "file.rs",
            "src/old/name.rs -> src/new/deep/name.rs",
        ];
        for path in paths {
            for max in [0, 1, 4, 8, 12, 15, 20, 24, 40, 80] {
                let shown = fit_path(path, max);
                assert!(width(&shown) <= max, "{path} @ {max} => {shown}");
                if width(path) <= max {
                    assert_eq!(shown, path);
                }
            }
        }
    }
}
