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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub y: u16,
    pub x: u16,
    pub text: String,
    pub code: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Screen {
    origin_x: u16,
    origin_y: u16,
    width: u16,
    height: u16,
    cells: Vec<Vec<String>>,
    sources: Vec<SourceSpan>,
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
            sources: Vec::new(),
        }
    }

    pub fn set_sources(&mut self, sources: Vec<SourceSpan>) {
        self.sources = sources;
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
        if let Some(text) = self.source_text(selection) {
            return text;
        }
        self.raw_text(selection)
    }

    fn source_text(&self, selection: Selection) -> Option<String> {
        let (start, end) = selection.ordered();
        if start.1 == end.1 || self.sources.is_empty() {
            return None;
        }
        let mut lines = Vec::new();
        let mut saw_code = false;
        for y in start.1..=end.1 {
            let Some(span) = self.sources.iter().find(|span| span.y == y && span.code) else {
                continue;
            };
            let (left, right) = selection.col_bounds(y)?;
            if right < span.x {
                continue;
            }
            saw_code = true;
            lines.push(copied_source(span, left, right));
        }
        if !saw_code {
            return None;
        }
        while lines.first().is_some_and(String::is_empty) {
            lines.remove(0);
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        Some(lines.join("\n"))
    }

    fn raw_text(&self, selection: Selection) -> String {
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

    pub fn word_span(&self, x: u16, y: u16) -> Option<((u16, u16), (u16, u16))> {
        let row = self.row(y)?;
        let col = symbol_start(row, x.checked_sub(self.origin_x)? as usize)?;
        let class = cell_class(&row[col])?;
        let mut start = col;
        while let Some(prev) = prev_symbol(row, start) {
            if cell_class(&row[prev]) != Some(class) {
                break;
            }
            start = prev;
        }
        let mut end = col;
        while let Some(next) = next_symbol(row, end) {
            if cell_class(&row[next]) != Some(class) {
                break;
            }
            end = next;
        }
        let end_col = end + symbol_width(&row[end]) - 1;
        Some((
            (self.origin_x + start as u16, y),
            (self.origin_x + end_col as u16, y),
        ))
    }

    pub fn line_span(&self, y: u16) -> Option<((u16, u16), (u16, u16))> {
        let row = self.row(y)?;
        let mut start = None;
        let mut end_col = 0;
        let mut col = 0;
        while col < row.len() {
            if row[col].is_empty() {
                col += 1;
                continue;
            }
            if cell_class(&row[col]).is_some() {
                if start.is_none() {
                    start = Some(col);
                }
                end_col = col + symbol_width(&row[col]) - 1;
            }
            col += symbol_width(&row[col]);
        }
        let start = start?;
        Some((
            (self.origin_x + start as u16, y),
            (self.origin_x + end_col as u16, y),
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellClass {
    Word,
    Punct,
}

fn copied_source(span: &SourceSpan, left: u16, right: u16) -> String {
    let shown = span.text.replace('\t', "    ");
    let shown_w = UnicodeWidthStr::width(shown.as_str());
    let start = usize::from(left.saturating_sub(span.x));
    let end = usize::from(right.saturating_sub(span.x));
    if start == 0 && end.saturating_add(1) >= shown_w {
        return span.text.clone();
    }
    slice_cols(&shown, start, end).trim_end().to_string()
}

fn slice_cols(text: &str, start: usize, end: usize) -> String {
    let mut col = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if col > end {
            break;
        }
        if col <= end && col + width > start {
            out.push(ch);
        }
        col += width;
    }
    out
}

fn cell_class(symbol: &str) -> Option<CellClass> {
    let ch = symbol.chars().next()?;
    if ch.is_whitespace() {
        return None;
    }
    if ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.') {
        Some(CellClass::Word)
    } else {
        Some(CellClass::Punct)
    }
}

fn symbol_width(symbol: &str) -> usize {
    UnicodeWidthStr::width(symbol).max(1)
}

fn symbol_start(row: &[String], col: usize) -> Option<usize> {
    if row.is_empty() || col >= row.len() {
        return None;
    }
    let mut index = col;
    while index > 0 && row[index].is_empty() {
        index -= 1;
    }
    if row[index].is_empty() {
        None
    } else {
        Some(index)
    }
}

fn prev_symbol(row: &[String], start: usize) -> Option<usize> {
    if start == 0 {
        return None;
    }
    symbol_start(row, start - 1).filter(|prev| *prev < start)
}

fn next_symbol(row: &[String], start: usize) -> Option<usize> {
    let next = start + symbol_width(&row[start]);
    symbol_start(row, next).filter(|index| *index > start)
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

    #[test]
    fn double_click_selects_a_word_and_triple_click_selects_a_line() {
        let screen = screen(&["foo bar_baz", "src/files.rs"], 12);
        let (anchor, head) = screen.word_span(5, 0).unwrap();
        assert_eq!(screen.selected_text(sel(anchor, head)), "bar_baz");
        let (anchor, head) = screen.word_span(4, 1).unwrap();
        assert_eq!(screen.selected_text(sel(anchor, head)), "files.rs");
        assert!(screen.word_span(3, 0).is_none());
        let (anchor, head) = screen.line_span(1).unwrap();
        assert_eq!(screen.selected_text(sel(anchor, head)), "src/files.rs");
    }

    #[test]
    fn multi_line_diff_copy_drops_gutter() {
        let mut screen = screen(
            &[
                "files.rs │   1   2 fn foo() {",
                "          │   3   4     bar()",
                "          │@@ hunk",
            ],
            40,
        );
        screen.set_sources(vec![
            SourceSpan {
                y: 0,
                x: 12,
                text: "fn foo() {".to_string(),
                code: true,
            },
            SourceSpan {
                y: 1,
                x: 12,
                text: "    bar()".to_string(),
                code: true,
            },
            SourceSpan {
                y: 2,
                x: 10,
                text: "@@ hunk".to_string(),
                code: false,
            },
        ]);
        assert_eq!(
            screen.selected_text(sel((0, 0), (30, 2))),
            "fn foo() {\n    bar()"
        );
        assert_eq!(
            screen.selected_text(sel((15, 0), (18, 1))),
            "foo() {\n    bar"
        );
    }
}
