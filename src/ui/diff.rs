use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::model::{DiffLine, DiffLineKind, FileDiff, ViewState};
use crate::select::SourceSpan;

use super::text::{
    self, clamp_scroll, draw, draw_stats, fill, stats_width, status_fg, style, truncate, visible,
};
use super::theme::{ACCENT, ADD_BG, CANVAS, DEL_BG, FG, GRAY, GREEN, PANEL, RED};

pub const GUTTER_WIDTH: u16 = 9;

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) -> Vec<SourceSpan> {
    if area.is_empty() {
        return Vec::new();
    }
    let [title, body] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    fill(frame, title, style(FG, PANEL));
    fill(frame, body, style(FG, CANVAS));
    let Some(file) = view.files.get(view.selected) else {
        draw(
            frame,
            title.x.saturating_add(1),
            title.y,
            "no changes",
            title.width.saturating_sub(1) as usize,
            style(GRAY, PANEL),
        );
        return Vec::new();
    };
    render_title(frame, title, file);
    render_body(frame, body, file, view.scroll)
}

fn render_title(frame: &mut Frame, area: Rect, file: &FileDiff) {
    if area.width < 2 {
        return;
    }
    let x = area.x + 1;
    let row_w = (area.width - 1) as usize;
    let glyph = file.status.glyph().to_string();
    draw(
        frame,
        x,
        area.y,
        &glyph,
        1,
        style(status_fg(file.status), PANEL),
    );
    let stats_w = stats_width(file.additions, file.deletions);
    let show_stats = row_w > stats_w + 4;
    let reserve = if show_stats { stats_w + 1 } else { 0 };
    let path_w = row_w.saturating_sub(2 + reserve);
    let path = truncate(&file.display_path(), path_w);
    draw(frame, x + 2, area.y, &path, path_w, style(FG, PANEL));
    if show_stats {
        draw_stats(
            frame,
            x + row_w as u16,
            area.y,
            file.additions,
            file.deletions,
            PANEL,
        );
    }
}

fn render_body(frame: &mut Frame, area: Rect, file: &FileDiff, scroll: usize) -> Vec<SourceSpan> {
    if area.is_empty() {
        return Vec::new();
    }
    let lines = visible_lines(file);
    let viewport = area.height as usize;
    if lines.is_empty() {
        draw(
            frame,
            area.x.saturating_add(1),
            area.y,
            "no changes",
            area.width.saturating_sub(1) as usize,
            style(GRAY, CANVAS),
        );
        return Vec::new();
    }
    let scroll = clamp_scroll(scroll, lines.len(), viewport);
    let show_bar = lines.len() > viewport && area.width > 1;
    let content_w = if show_bar {
        area.width.saturating_sub(1)
    } else {
        area.width
    };
    let rows = viewport.min(lines.len().saturating_sub(scroll));
    let mut spans = Vec::with_capacity(rows);
    for row in 0..rows {
        let line = lines[scroll + row];
        let y = area.y + row as u16;
        render_line(frame, area, row as u16, line, content_w);
        let code = matches!(
            line.kind,
            DiffLineKind::Add | DiffLineKind::Del | DiffLineKind::Context
        );
        spans.push(SourceSpan {
            y,
            x: if code {
                area.x.saturating_add(GUTTER_WIDTH)
            } else {
                area.x
            },
            text: line.text.clone(),
            code,
        });
    }
    if show_bar {
        text::scrollbar(frame, area, lines.len(), scroll, viewport, CANVAS);
    }
    spans
}

fn visible_lines(file: &FileDiff) -> Vec<&DiffLine> {
    if file.binary {
        file.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::Meta)
            .collect()
    } else {
        file.lines.iter().collect()
    }
}

fn render_line(frame: &mut Frame, area: Rect, row: u16, line: &DiffLine, content_w: u16) {
    let y = area.y + row;
    let (fg, bg) = line_colors(line.kind);
    fill(
        frame,
        Rect {
            x: area.x,
            y,
            width: content_w,
            height: 1,
        },
        style(fg, bg),
    );
    let text = visible(&line.text);
    match line.kind {
        DiffLineKind::Hunk | DiffLineKind::Meta => {
            draw(frame, area.x, y, &text, content_w as usize, style(fg, bg));
        }
        kind => {
            let gutter = format!(
                "{}{}{}",
                lineno(line.old_lineno),
                lineno(line.new_lineno),
                sign(kind)
            );
            let gutter_w = usize::from(GUTTER_WIDTH).min(content_w as usize);
            draw(frame, area.x, y, &gutter, gutter_w, style(fg, bg));
            if content_w > GUTTER_WIDTH {
                draw(
                    frame,
                    area.x + GUTTER_WIDTH,
                    y,
                    &text,
                    content_w as usize - usize::from(GUTTER_WIDTH),
                    style(fg, bg),
                );
            }
        }
    }
}

fn line_colors(kind: DiffLineKind) -> (ratatui::style::Color, ratatui::style::Color) {
    match kind {
        DiffLineKind::Add => (GREEN, ADD_BG),
        DiffLineKind::Del => (RED, DEL_BG),
        DiffLineKind::Hunk => (ACCENT, CANVAS),
        DiffLineKind::Meta => (GRAY, CANVAS),
        DiffLineKind::Context => (FG, CANVAS),
    }
}

fn sign(kind: DiffLineKind) -> char {
    match kind {
        DiffLineKind::Add => '+',
        DiffLineKind::Del => '-',
        _ => ' ',
    }
}

fn lineno(n: Option<u32>) -> String {
    match n {
        Some(n) => {
            let text = n.to_string();
            if text.len() <= 4 {
                format!("{text:>4}")
            } else {
                text[text.len() - 4..].to_string()
            }
        }
        None => "    ".to_string(),
    }
}
