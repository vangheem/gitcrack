use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::model::ViewState;

use super::text::{draw, fill, style, truncate, width};
use super::theme::{ACCENT, FG, GREEN, PANEL, RED};

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) {
    fill(frame, area, style(FG, PANEL));
    if area.width < 2 {
        return;
    }
    let x = area.x + 1;
    let right = area.right() - 1;
    let usable = (right - x) as usize;
    let (adds, dels) = view.stat_summary();
    let adds_s = format!("+{adds}");
    let dels_s = format!("-{dels}");
    let stats_w = width(&adds_s) + 1 + width(&dels_s);
    let reserve = if usable > stats_w + 6 { stats_w + 2 } else { 0 };
    let repo = view.repo.name();
    let branch = view.repo.branch_label();
    let target = view.target.label();
    let brand = Style::new()
        .fg(ACCENT)
        .bg(PANEL)
        .add_modifier(Modifier::BOLD);
    let items = [
        ("gitcrack", brand),
        (repo.as_str(), style(FG, PANEL)),
        (branch.as_str(), style(FG, PANEL)),
        (target.as_str(), style(FG, PANEL)),
    ];
    draw_items(frame, x, area.y, usable.saturating_sub(reserve), &items);
    if reserve > 0 {
        let stats_x = right.saturating_sub(stats_w as u16);
        draw(
            frame,
            stats_x,
            area.y,
            &adds_s,
            width(&adds_s),
            style(GREEN, PANEL),
        );
        draw(
            frame,
            stats_x + width(&adds_s) as u16 + 1,
            area.y,
            &dels_s,
            width(&dels_s),
            style(RED, PANEL),
        );
    }
}

fn draw_items(frame: &mut Frame, x: u16, y: u16, budget: usize, items: &[(&str, Style)]) {
    let mut used = 0usize;
    let mut cursor = x;
    for (index, (text, paint)) in items.iter().enumerate() {
        if used >= budget {
            break;
        }
        let gap = if index == 0 { 0 } else { 2 };
        if used + gap >= budget {
            break;
        }
        let avail = budget - used - gap;
        let shown = truncate(text, avail);
        if shown.is_empty() {
            break;
        }
        cursor = cursor.saturating_add(gap as u16);
        used += gap;
        draw(frame, cursor, y, &shown, avail, *paint);
        let drawn = width(&shown);
        cursor = cursor.saturating_add(drawn as u16);
        used += drawn;
        if width(text) > avail {
            break;
        }
    }
}
