use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Fill;

use crate::model::{FileDiff, ViewState};

use super::text::{
    dim, draw, draw_stats, fill, fit_path, stats_width, status_fg, style, width, window_start,
};
use super::theme::{BORDER, CANVAS, FG, GRAY, SELECTED};

pub fn pane_width(body_width: u16, view: &ViewState) -> u16 {
    const PREFERRED_MIN: u16 = 48;
    const HARD_MAX: u16 = 80;
    if body_width == 0 {
        return 0;
    }
    let max = (body_width / 2).clamp(1, HARD_MAX).min(body_width);
    let min = PREFERRED_MIN.min(max);
    let desired = view
        .files
        .iter()
        .map(desired_pane_width)
        .max()
        .unwrap_or(0)
        .min(usize::from(u16::MAX)) as u16;
    desired.clamp(min, max)
}

fn desired_pane_width(file: &FileDiff) -> usize {
    let path_w = width(&file.display_path());
    let marker = file.kind.marker();
    let marker_w = if marker.is_empty() {
        0
    } else {
        width(marker) + 1
    };
    let stats_reserve = stats_width(file.additions, file.deletions) + 1;
    let inset = 1;
    let glyph_gap = 2;
    let border = 1;
    path_w + marker_w + stats_reserve + inset + glyph_gap + border
}

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) {
    if area.is_empty() {
        return;
    }
    let [list, rule] = Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
    fill(frame, list, style(FG, CANVAS));
    frame.render_widget(Fill::new("│").style(style(BORDER, CANVAS)), rule);
    if view.files.is_empty() {
        draw(
            frame,
            list.x.saturating_add(1),
            list.y,
            "no files",
            list.width.saturating_sub(1) as usize,
            style(GRAY, CANVAS),
        );
        return;
    }
    let visible = list.height as usize;
    let start = window_start(view.selected, view.files.len(), visible);
    let rows = visible.min(view.files.len().saturating_sub(start));
    for row in 0..rows {
        let index = start + row;
        let y = list.y + row as u16;
        render_row(frame, list, y, &view.files[index], index == view.selected);
    }
}

fn render_row(frame: &mut Frame, area: Rect, y: u16, file: &FileDiff, selected: bool) {
    let bg = if selected { SELECTED } else { CANVAS };
    fill(
        frame,
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
        style(FG, bg),
    );
    if area.width == 0 {
        return;
    }
    let inset = u16::from(area.width > 1);
    let x = area.x + inset;
    let row_w = area.width.saturating_sub(inset) as usize;
    if row_w == 0 {
        return;
    }
    let glyph = file.status.glyph().to_string();
    draw(frame, x, y, &glyph, 1, style(status_fg(file.status), bg));
    if row_w < 3 {
        return;
    }
    let stats_w = stats_width(file.additions, file.deletions);
    let show_stats = row_w > stats_w + 4;
    let stats_reserve = if show_stats { stats_w + 1 } else { 0 };
    let marker = file.kind.marker();
    let marker_w = if marker.is_empty() {
        0
    } else {
        width(marker) + 1
    };
    let mut path_budget = row_w.saturating_sub(2 + stats_reserve);
    let show_marker = !marker.is_empty() && path_budget > marker_w;
    if show_marker {
        path_budget -= marker_w;
    }
    let path = fit_path(&file.display_path(), path_budget);
    draw(frame, x + 2, y, &path, path_budget, style(FG, bg));
    if show_marker {
        let marker_x = x + 2 + width(&path) as u16 + 1;
        draw(frame, marker_x, y, marker, width(marker), dim(GRAY, bg));
    }
    if show_stats {
        draw_stats(
            frame,
            x + row_w as u16,
            y,
            file.additions,
            file.deletions,
            bg,
        );
    }
}
