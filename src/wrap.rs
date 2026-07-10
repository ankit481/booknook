//! Word-wrapping parsed blocks to a specific column width.
//!
//! ratatui's own `Paragraph` widget can wrap text, but it gives no way to
//! see the individual wrapped rows it produces, which means there is no
//! way to insert a blank row between them for real line spacing. This
//! module does the wrapping itself instead, so the reader can space every
//! line the way an e-reader's line-spacing setting would.

use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use unicode_width::UnicodeWidthStr;

use crate::markdown::RenderLine;

/// Lay out parsed blocks at a specific column width. Every prose block is
/// word-wrapped, with a blank row inserted between its wrapped lines for
/// real line spacing, while verbatim blocks such as code lines and
/// separators pass through unchanged.
pub(crate) fn layout(blocks: &[RenderLine], width: u16) -> Text<'static> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for block in blocks {
        match block {
            RenderLine::Verbatim(line) => out.push(line.clone()),
            RenderLine::Prose { spans, hang } => {
                for (i, row) in wrap_prose(spans, *hang, width).into_iter().enumerate() {
                    if i > 0 {
                        out.push(Line::default());
                    }
                    out.push(row);
                }
            }
        }
    }
    Text::from(out)
}

/// Word-wrap one block's spans into rows that fit `width` columns, keeping
/// each word's original style. Continuation rows, meaning every row after
/// the first, are indented by `hang` columns, so a wrapped list item's
/// second line lands under its text instead of under the bullet.
fn wrap_prose(spans: &[Span<'static>], hang: u16, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1) as usize;
    let hang = hang as usize;

    let words: Vec<(&str, Style)> = spans
        .iter()
        .flat_map(|span| span.content.split_whitespace().map(|w| (w, span.style)))
        .collect();
    if words.is_empty() {
        return vec![Line::default()];
    }

    let row_indent = |row_index: usize| if row_index == 0 { 0 } else { hang };

    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for (word, style) in words {
        let word_width = UnicodeWidthStr::width(word);
        let available = width.saturating_sub(row_indent(rows.len())).max(1);
        let needed = if current_width == 0 {
            word_width
        } else {
            current_width + 1 + word_width
        };

        if needed > available && current_width > 0 {
            rows.push(indent_line(std::mem::take(&mut current), row_indent(rows.len())));
            current_width = 0;
        }
        if current_width > 0 {
            current.push(Span::raw(" "));
            current_width += 1;
        }
        current.push(Span::styled(word.to_string(), style));
        current_width += word_width;
    }
    if !current.is_empty() {
        rows.push(indent_line(current, row_indent(rows.len())));
    }
    rows
}

fn indent_line(mut spans: Vec<Span<'static>>, indent: usize) -> Line<'static> {
    if indent > 0 {
        spans.insert(0, Span::raw(" ".repeat(indent)));
    }
    Line::from(spans)
}
