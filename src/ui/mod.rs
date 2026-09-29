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

use self::text::style;
use self::theme::{FG, GRAY, PANEL};

pub fn render(frame: &mut Frame, view: &ViewState) {
    let area = frame.area();
    if area.height < 20 || area.width < 50 {
        render_too_small(frame, area);
        return;
    }
    let [header_area, body, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);
    header::render(frame, header_area, view);
    let [file_area, diff_area] =
        Layout::horizontal([Constraint::Length(36), Constraint::Fill(1)]).areas(body);
    files::render(frame, file_area, view);
    diff::render(frame, diff_area, view);
    footer::render(frame, footer_area, view);
    if view.overlay != Overlay::None {
        overlay::render(frame, area, view);
    }
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
