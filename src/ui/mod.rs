mod comment;
mod diff;
mod files;
mod footer;
mod header;
mod overlay;
mod text;
mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Fill, Paragraph};

use crate::model::{Overlay, ViewState};
use crate::select::SourceSpan;

use self::text::style;
use self::theme::{FG, GRAY, PANEL};

pub use self::text::paint_selection;

pub fn render(frame: &mut Frame, view: &ViewState) -> Vec<SourceSpan> {
    let area = frame.area();
    if area.height < 20 || area.width < 50 {
        render_too_small(frame, area);
        return Vec::new();
    }
    let comment_h = view.comment_rows();
    let [header_area, body, comment_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(comment_h),
        Constraint::Length(1),
    ])
    .areas(area);
    header::render(frame, header_area, view);
    let file_w = files::pane_width(body.width, view);
    let [file_area, diff_area] =
        Layout::horizontal([Constraint::Length(file_w), Constraint::Fill(1)]).areas(body);
    files::render(frame, file_area, view);
    let sources = diff::render(frame, diff_area, view);
    comment::render(frame, comment_area, view);
    footer::render(frame, footer_area, view);
    if view.overlay != Overlay::None {
        overlay::render(frame, area, view);
    }
    if let Some(message) = &view.loading {
        overlay::render_loading(frame, area, message);
    }
    sources
}

fn render_too_small(frame: &mut Frame, area: Rect) {
    frame.render_widget(Fill::new(" ").style(style(FG, PANEL)), area);
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new("terminal too small")
            .centered()
            .style(style(GRAY, PANEL)),
        mid,
    );
}
