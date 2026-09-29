use ratatui::buffer::Buffer;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: (u16, u16),
    pub head: (u16, u16),
}

impl Selection {
    pub fn ordered(self) -> ((u16, u16), (u16, u16)) {
        if (self.anchor.1, self.anchor.0) <= (self.head.1, self.head.0) {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn is_empty(self) -> bool {
        self.anchor == self.head
    }

    pub fn col_bounds(self, row: u16) -> Option<(u16, u16)> {
        let (start, end) = self.ordered();
        if row < start.1 || row > end.1 {
            return None;
        }
        let left = if row == start.1 { start.0 } else { 0 };
        let right = if row == end.1 { end.0 } else { u16::MAX };
        Some((left, right))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Screen {
    origin_x: u16,
    origin_y: u16,
    width: u16,
    height: u16,
    cells: Vec<Vec<String>>,
}

impl Screen {
    pub fn from_buffer(buffer: &Buffer) -> Self {
        let area = buffer.area;
        let mut cells = Vec::with_capacity(area.height as usize);
        for y in area.y..area.bottom() {
            let mut row = vec![String::new(); area.width as usize];
            let mut x = area.x;
            while x < area.right() {
                let Some(cell) = buffer.cell((x, y)) else {
                    break;
                };
                let symbol = cell.symbol();
                let width = UnicodeWidthStr::width(symbol).max(1) as u16;
                let col = (x - area.x) as usize;
                if col < row.len() {
                    row[col] = symbol.to_string();
                }
                let next = x.saturating_add(width);
                if next <= x {
                    break;
                }
                x = next;
            }
            cells.push(row);
        }
        Self {
            origin_x: area.x,
            origin_y: area.y,
            width: area.width,
            height: area.height,
            cells,
        }
    }

    pub fn clamp(&self, x: u16, y: u16) -> (u16, u16) {
        if self.width == 0 || self.height == 0 {
            return (self.origin_x, self.origin_y);
        }
        let x = x.clamp(self.origin_x, self.origin_x.saturating_add(self.width) - 1);
        let y = y.clamp(self.origin_y, self.origin_y.saturating_add(self.height) - 1);
        (x, y)
    }

    pub fn selected_text(&self, selection: Selection) -> String {
        let (start, end) = selection.ordered();
        if self.cells.is_empty() || start.1 > end.1 {
            return String::new();
        }
        let mut lines = Vec::new();
        for y in start.1..=end.1 {
            let Some(row) = self.row(y) else {
                continue;
            };
            let Some((left, right)) = selection.col_bounds(y) else {
                continue;
            };
            let left = left.saturating_sub(self.origin_x);
            let right = right.saturating_sub(self.origin_x);
            lines.push(slice_row(row, left, right).trim_end().to_string());
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines.join("\n")
    }

    fn row(&self, y: u16) -> Option<&[String]> {
        let index = y.checked_sub(self.origin_y)? as usize;
        self.cells.get(index).map(Vec::as_slice)
    }
}

fn slice_row(row: &[String], left: u16, right: u16) -> String {
    if row.is_empty() || left > right {
        return String::new();
    }
    let last = row.len() - 1;
    let left = (left as usize).min(last);
    let right = (right as usize).min(last);
    let mut start = left;
    while start > 0 && row[start].is_empty() {
        start -= 1;
    }
    let mut out = String::new();
    let mut col = start;
    while col <= right && col < row.len() {
        let symbol = &row[col];
        if symbol.is_empty() {
            col += 1;
            continue;
        }
        let width = UnicodeWidthStr::width(symbol.as_str()).max(1);
        out.push_str(symbol);
        col += width;
    }
    out
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use super::*;

    fn screen(lines: &[&str], width: u16) -> Screen {
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, lines.len() as u16));
        for (y, line) in lines.iter().enumerate() {
            buffer.set_string(0, y as u16, line, Style::default());
        }
        Screen::from_buffer(&buffer)
    }

    fn sel(anchor: (u16, u16), head: (u16, u16)) -> Selection {
        Selection { anchor, head }
    }

    #[test]
    fn copies_a_span_and_trims_padding() {
        let screen = screen(&["hi"], 8);
        assert_eq!(screen.selected_text(sel((0, 0), (7, 0))), "hi");
        assert_eq!(screen.selected_text(sel((1, 0), (1, 0))), "i");
    }

    #[test]
    fn copies_across_lines_and_wide_chars() {
        let screen = screen(&["ab字cd", "second", "tail"], 8);
        assert_eq!(
            screen.selected_text(sel((1, 0), (2, 2))),
            "b字cd\nsecond\ntai"
        );
        assert_eq!(screen.selected_text(sel((3, 0), (4, 0))), "字c");
    }

    #[test]
    fn reversed_drag_matches_forward_drag() {
        let screen = screen(&["abcdef", "ghijkl"], 6);
        assert_eq!(
            screen.selected_text(sel((4, 1), (1, 0))),
            screen.selected_text(sel((1, 0), (4, 1)))
        );
        assert_eq!(screen.selected_text(sel((1, 0), (4, 1))), "bcdef\nghijk");
    }
}
