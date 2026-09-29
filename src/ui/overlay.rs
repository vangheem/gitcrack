use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::model::{Overlay, ViewState};

use super::text::{self, centered, draw, fill, style, truncate, width, window_start};
use super::theme::{ACCENT, BORDER, FG, GRAY, GREEN, PANEL, SELECTED, YELLOW};

const HELP: &[&str] = &[
    "up/down or tab/shift-tab changes file",
    "shift-up/down or j/k scrolls",
    "page up/down pages",
    "space pages the diff",
    "ctrl-u/ctrl-d half-pages",
    "home/end jumps",
    "[ and ] also change file",
    "c, b, p open pickers",
    "w returns to the working tree",
    "r refreshes    q or ctrl-c quits",
    "picker: up/down moves, enter confirms",
    "space sets a commit range base",
    "drag selects text and copies it",
    "esc or q closes",
];

pub fn render(frame: &mut Frame, area: Rect, view: &ViewState) {
    if view.overlay == Overlay::None {
        return;
    }
    let popup = centered(area, 80, 70);
    frame.render_widget(Clear, popup);
    let title = match view.overlay {
        Overlay::Commits => "commits",
        Overlay::Branches => "branches",
        Overlay::PullRequests => "pull requests",
        Overlay::Help => "help",
        Overlay::None => return,
    };
    let mut block = Block::bordered()
        .border_style(style(BORDER, PANEL))
        .style(style(FG, PANEL))
        .title(Line::from(title).style(style(ACCENT, PANEL).add_modifier(Modifier::BOLD)));
    if view.overlay == Overlay::Commits {
        if let Some(base) = &view.pending_base {
            let footer = commit_footer(base, popup.width.saturating_sub(2) as usize);
            block = block.title_bottom(Line::from(footer).style(style(GRAY, PANEL)));
        }
    }
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    match view.overlay {
        Overlay::Commits => render_commits(frame, inner, view),
        Overlay::Branches => render_branches(frame, inner, view),
        Overlay::PullRequests => render_prs(frame, inner, view),
        Overlay::Help => render_help(frame, inner),
        Overlay::None => {}
    }
}

fn commit_footer(base: &str, max: usize) -> String {
    let suffix = "  space sets base  enter opens commit or range";
    let full = format!("base {base}{suffix}");
    if width(&full) <= max {
        return full;
    }
    let fixed = width("base ") + width(suffix);
    if max <= fixed {
        return truncate(&full, max);
    }
    let sha = truncate(base, max - fixed);
    format!("base {sha}{suffix}")
}

fn render_help(frame: &mut Frame, area: Rect) {
    let area = if area.width >= 40 {
        Rect {
            x: area.x + 1,
            width: area.width.saturating_sub(2),
            ..area
        }
    } else {
        area
    };
    let lines = HELP
        .iter()
        .map(|line| Line::from(*line))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(style(FG, PANEL)),
        area,
    );
}

fn render_commits(frame: &mut Frame, area: Rect, view: &ViewState) {
    if view.commits.is_empty() {
        empty(frame, area, "no commits");
        return;
    }
    render_picker(
        frame,
        area,
        view.commits.len(),
        view.overlay_selected,
        |frame, x, y, w, index, bg| {
            draw_commit(frame, x, y, w, view, index, bg);
        },
    );
}

fn render_branches(frame: &mut Frame, area: Rect, view: &ViewState) {
    if view.branches.is_empty() {
        empty(frame, area, "no branches");
        return;
    }
    render_picker(
        frame,
        area,
        view.branches.len(),
        view.overlay_selected,
        |frame, x, y, w, index, bg| {
            draw_branch(frame, x, y, w, view, index, bg);
        },
    );
}

fn render_prs(frame: &mut Frame, area: Rect, view: &ViewState) {
    if view.prs.is_empty() {
        empty(frame, area, "no open pull requests");
        return;
    }
    render_picker(
        frame,
        area,
        view.prs.len(),
        view.overlay_selected,
        |frame, x, y, w, index, bg| {
            draw_pr(frame, x, y, w, view, index, bg);
        },
    );
}

fn render_picker(
    frame: &mut Frame,
    area: Rect,
    len: usize,
    selected: usize,
    mut draw_item: impl FnMut(&mut Frame, u16, u16, usize, usize, ratatui::style::Color),
) {
    if area.is_empty() || len == 0 {
        return;
    }
    let visible = area.height as usize;
    let start = window_start(selected, len, visible);
    let bar = len > visible && area.width > 2;
    let left = u16::from(area.width > 2);
    let right = if bar { 1 } else { u16::from(area.width > 2) };
    let text_w = area.width.saturating_sub(left + right) as usize;
    let rows = visible.min(len.saturating_sub(start));
    for row in 0..rows {
        let index = start + row;
        let y = area.y + row as u16;
        let bg = if index == selected { SELECTED } else { PANEL };
        fill(
            frame,
            Rect {
                x: area.x,
                y,
                width: area.width.saturating_sub(u16::from(bar)),
                height: 1,
            },
            style(FG, bg),
        );
        draw_item(frame, area.x + left, y, text_w, index, bg);
    }
    if bar {
        text::scrollbar(frame, area, len, start, visible, PANEL);
    }
}

fn draw_commit(
    frame: &mut Frame,
    x: u16,
    y: u16,
    row_w: usize,
    view: &ViewState,
    index: usize,
    bg: ratatui::style::Color,
) {
    let Some(commit) = view.commits.get(index) else {
        return;
    };
    let sha = truncate(&commit.short_sha, row_w.min(12));
    draw(frame, x, y, &sha, row_w, style(ACCENT, bg));
    let mut used = width_of_gap(&sha);
    let rest = row_w.saturating_sub(used);
    let subject_min = 8.min(rest);
    let mut budget = rest.saturating_sub(subject_min);
    let date = if budget > 2 {
        let shown = truncate(&commit.date, (budget - 2).min(16));
        let w = width(&shown) + 2;
        if shown.is_empty() {
            String::new()
        } else {
            budget = budget.saturating_sub(w);
            shown
        }
    } else {
        String::new()
    };
    let author = if budget > 2 {
        let shown = truncate(&commit.author, (budget - 2).min(16));
        if shown.is_empty() {
            String::new()
        } else {
            shown
        }
    } else {
        String::new()
    };
    if !date.is_empty() {
        draw(
            frame,
            x + used as u16,
            y,
            &date,
            width(&date),
            style(GRAY, bg),
        );
        used += width(&date) + 2;
    }
    if !author.is_empty() {
        draw(
            frame,
            x + used as u16,
            y,
            &author,
            width(&author),
            style(GRAY, bg),
        );
        used += width(&author) + 2;
    }
    let subject_w = row_w.saturating_sub(used);
    let subject = truncate(&commit.subject, subject_w);
    draw(
        frame,
        x + used as u16,
        y,
        &subject,
        subject_w,
        style(FG, bg),
    );
}

fn width_of_gap(text: &str) -> usize {
    let w = width(text);
    if w == 0 { 0 } else { w + 2 }
}

fn draw_branch(
    frame: &mut Frame,
    x: u16,
    y: u16,
    row_w: usize,
    view: &ViewState,
    index: usize,
    bg: ratatui::style::Color,
) {
    let Some(branch) = view.branches.get(index) else {
        return;
    };
    let vs = format!("vs {}", view.repo.default_branch);
    let head = if branch.is_head { "HEAD" } else { "" };
    let vs_w = width(&vs);
    let head_w = if head.is_empty() { 0 } else { width(head) + 2 };
    let show_vs = row_w > vs_w + head_w + 4;
    let show_head = !head.is_empty() && row_w > head_w + 4 + usize::from(show_vs) * (vs_w + 2);
    let name_w = row_w
        .saturating_sub(usize::from(show_vs) * (vs_w + 2))
        .saturating_sub(usize::from(show_head) * head_w);
    let name = truncate(&branch.name, name_w);
    draw(frame, x, y, &name, name_w, style(FG, bg));
    if show_head {
        let head_x = x + width(&name) as u16 + 2;
        draw(frame, head_x, y, head, width(head), style(GREEN, bg));
    }
    if show_vs {
        let vs_x = x + row_w as u16 - vs_w as u16;
        draw(frame, vs_x, y, &vs, vs_w, dim_gray(bg));
    }
}

fn draw_pr(
    frame: &mut Frame,
    x: u16,
    y: u16,
    row_w: usize,
    view: &ViewState,
    index: usize,
    bg: ratatui::style::Color,
) {
    let Some(pr) = view.prs.get(index) else {
        return;
    };
    let mut cursor = 0usize;
    if pr.is_draft && row_w > 0 {
        let draft = "draft ";
        let shown = truncate(draft, row_w);
        draw(frame, x, y, &shown, row_w, style(YELLOW, bg));
        cursor += width(&shown);
    }
    let number = format!("#{}", pr.number);
    if cursor < row_w {
        let shown = truncate(&number, row_w - cursor);
        draw(
            frame,
            x + cursor as u16,
            y,
            &shown,
            row_w - cursor,
            style(ACCENT, bg),
        );
        cursor += width(&shown);
    }
    let refs = format!("{} -> {}", pr.head_ref, pr.base_ref);
    let author = truncate(&pr.author, 16);
    let refs_shown = truncate(&refs, 28);
    let right_w = if author.is_empty() {
        width(&refs_shown)
    } else {
        width(&author) + 2 + width(&refs_shown)
    };
    let title_at = cursor + usize::from(cursor > 0);
    let show_right = !refs_shown.is_empty() && row_w > title_at + right_w + 4;
    let title_w = if show_right {
        row_w.saturating_sub(title_at + 2 + right_w)
    } else {
        row_w.saturating_sub(title_at)
    };
    if title_at < row_w && title_w > 0 {
        let title = truncate(&pr.title, title_w);
        draw(
            frame,
            x + title_at as u16,
            y,
            &title,
            title_w,
            style(FG, bg),
        );
    }
    if show_right {
        let mut right_x = x + row_w as u16 - right_w as u16;
        if !author.is_empty() {
            draw(frame, right_x, y, &author, width(&author), style(GRAY, bg));
            right_x = right_x.saturating_add(width(&author) as u16 + 2);
        }
        draw(
            frame,
            right_x,
            y,
            &refs_shown,
            width(&refs_shown),
            style(GRAY, bg),
        );
    }
}

fn dim_gray(bg: ratatui::style::Color) -> ratatui::style::Style {
    text::dim(GRAY, bg)
}

fn empty(frame: &mut Frame, area: Rect, message: &str) {
    if area.is_empty() {
        return;
    }
    let shown = truncate(message, area.width as usize);
    let w = width(&shown) as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height / 2;
    draw(frame, x, y, &shown, area.width as usize, style(GRAY, PANEL));
}
