//! Word-wrapping parsed blocks to a specific column width.
//!
//! ratatui's own `Paragraph` widget can wrap text, but it gives no way to
//! see the individual wrapped rows it produces, which means there is no
//! way to insert a blank row between them for line spacing. This module
//! does the wrapping itself instead, so the reader can space every line
//! the way an e-reader's line-spacing setting would.

use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::markdown::RenderLine;

/// One word, as a run of styled pieces.
///
/// A word is not the same thing as a span. The markdown parser emits smart
/// punctuation as its own event, so the word `country's` arrives as three
/// separate spans: `country`, then `’`, then `s`. Treating each span as a
/// word would put a space on either side of the apostrophe. A word is
/// therefore whatever sits between two runs of whitespace, no matter how
/// many spans and styles it crosses.
type Word = Vec<(String, Style)>;

/// How much vertical air to leave between lines and between blocks.
///
/// These are two different numbers on purpose. If the gap inside a
/// paragraph matched the gap between paragraphs, every line would look
/// like its own paragraph, and the page would lose all of its structure.
#[derive(Clone, Copy)]
pub(crate) struct Spacing {
    /// Blank rows between the wrapped lines of a single block.
    pub(crate) line: u16,
    /// Blank rows between one block and the next.
    pub(crate) paragraph: u16,
}

/// A laid-out document: the terminal rows to draw, and where each input
/// block begins among them.
///
/// `block_rows` has one entry per input block, giving the row in `text` at
/// which that block's content starts. It is what lets the reader jump to a
/// heading: the sidebar knows a heading's block index, and this maps that
/// index to a row, which divides by the viewport height to give a page.
/// Because layout runs every frame at the current width, this mapping is
/// always correct for the width on screen right now.
pub(crate) struct Laid {
    pub(crate) text: Text<'static>,
    pub(crate) block_rows: Vec<u16>,
}

/// Lay out parsed blocks at a specific column width. Every prose block is
/// word-wrapped and spaced according to `spacing`. Verbatim blocks, such as
/// code and ASCII diagrams, keep their exact shape but are clipped to the
/// column so a wide one is cut cleanly at the edge rather than soft-wrapped
/// into a second row, which would break its alignment.
pub(crate) fn layout(blocks: &[RenderLine], width: u16, spacing: Spacing) -> Laid {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut block_rows: Vec<u16> = Vec::with_capacity(blocks.len());
    let mut prev_was_gap = false;

    for block in blocks {
        // The row this block starts on is wherever output currently ends. A
        // collapsed gap adds nothing and simply reports the current row, so
        // the entry stays aligned with `blocks` one-for-one.
        block_rows.push(out.len().min(u16::MAX as usize) as u16);
        match block {
            // Two blocks in a row can each ask for a gap, for instance a
            // paragraph ending just before its enclosing list does. Only
            // the first one gets to draw it.
            RenderLine::Gap => {
                if !prev_was_gap && !out.is_empty() {
                    for _ in 0..spacing.paragraph {
                        out.push(Line::default());
                    }
                }
                prev_was_gap = true;
                continue;
            }
            RenderLine::Verbatim(line) => out.push(clip_line(line, width as usize)),
            RenderLine::Prose { spans, indent, hang } => {
                for (i, row) in wrap_prose(spans, *indent, *hang, width).into_iter().enumerate() {
                    if i > 0 {
                        for _ in 0..spacing.line {
                            out.push(Line::default());
                        }
                    }
                    out.push(row);
                }
            }
        }
        prev_was_gap = false;
    }
    Laid { text: Text::from(out), block_rows }
}

/// Break a block's spans into words, where a word is a run of non-whitespace
/// text that may cross span boundaries and carry more than one style.
fn split_words(spans: &[Span<'static>]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut current: Word = Vec::new();

    for span in spans {
        let style = span.style;
        let mut rest: &str = span.content.as_ref();
        while !rest.is_empty() {
            // Walk one run at a time, alternating between whitespace and
            // non-whitespace, so a word can be assembled across as many
            // spans as it takes.
            let leading_is_space = rest.starts_with(char::is_whitespace);
            let end = rest
                .find(|c: char| c.is_whitespace() != leading_is_space)
                .unwrap_or(rest.len());
            let (chunk, tail) = rest.split_at(end);

            if leading_is_space {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            } else {
                current.push((chunk.to_string(), style));
            }
            rest = tail;
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn word_width(word: &Word) -> usize {
    word.iter().map(|(text, _)| UnicodeWidthStr::width(text.as_str())).sum()
}

/// Word-wrap one block's spans into rows that fit `width` columns, keeping
/// each word's original styling. The first row is indented by `indent`
/// columns, and every row after it by `hang`, so a wrapped list item's
/// second line lands under its text instead of under the bullet.
fn wrap_prose(spans: &[Span<'static>], indent: u16, hang: u16, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1) as usize;
    let (indent, hang) = (indent as usize, hang as usize);

    let words = split_words(spans);
    if words.is_empty() {
        return vec![Line::default()];
    }

    let row_indent = |row_index: usize| if row_index == 0 { indent } else { hang };

    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for word in words {
        let this_width = word_width(&word);
        let available = width.saturating_sub(row_indent(rows.len())).max(1);
        let needed = if current_width == 0 {
            this_width
        } else {
            current_width + 1 + this_width
        };

        if needed > available && current_width > 0 {
            rows.push(indent_line(std::mem::take(&mut current), row_indent(rows.len())));
            current_width = 0;
        }
        if current_width > 0 {
            current.push(Span::raw(" "));
            current_width += 1;
        }
        for (text, style) in word {
            current_width += UnicodeWidthStr::width(text.as_str());
            current.push(Span::styled(text, style));
        }
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

/// Cut a finished line down to `width` columns, keeping each kept piece's
/// style. This is what stops a wide code line or ASCII diagram from being
/// soft-wrapped into a second row and losing its alignment: it is clipped at
/// the edge instead, with a `›` marking where content was dropped so a
/// clipped line reads as clipped rather than as simply short. A line that
/// already fits passes through untouched.
fn clip_line(line: &Line<'static>, width: usize) -> Line<'static> {
    let total: usize = line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
    if total <= width {
        return line.clone();
    }

    // One column is held back for the cut marker, so the clipped line ends up
    // exactly `width` wide, marker included.
    let limit = width.saturating_sub(1);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for span in &line.spans {
        if used >= limit {
            break;
        }
        let mut kept = String::new();
        for ch in span.content.chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + ch_width > limit {
                break;
            }
            kept.push(ch);
            used += ch_width;
        }
        if !kept.is_empty() {
            spans.push(Span::styled(kept, span.style));
        }
    }
    let marker_style = line.spans.last().map(|s| s.style).unwrap_or_default();
    spans.push(Span::styled("›".to_string(), marker_style));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn wrap_to_strings(spans: &[Span<'static>], width: u16) -> Vec<String> {
        wrap_prose(spans, 0, 0, width).iter().map(text_of).collect()
    }

    /// The parser emits smart punctuation as its own span, so an
    /// apostrophe splits `country's` into three. It must still render as
    /// one word, with no spaces introduced around the apostrophe.
    #[test]
    fn joins_words_split_across_spans() {
        let spans = vec![Span::raw("the country"), Span::raw("’"), Span::raw("s fight")];
        assert_eq!(wrap_to_strings(&spans, 40), vec!["the country’s fight"]);
    }

    #[test]
    fn joins_currency_split_across_spans() {
        let spans = vec![Span::raw("at "), Span::raw("$"), Span::raw("78.02")];
        assert_eq!(wrap_to_strings(&spans, 40), vec!["at $78.02"]);
    }

    #[test]
    fn collapses_runs_of_whitespace_between_words() {
        let spans = vec![Span::raw("a  b"), Span::raw("   c")];
        assert_eq!(wrap_to_strings(&spans, 40), vec!["a b c"]);
    }

    #[test]
    fn wraps_at_the_column_width() {
        let spans = vec![Span::raw("alpha beta gamma")];
        assert_eq!(wrap_to_strings(&spans, 10), vec!["alpha beta", "gamma"]);
    }

    /// Continuation rows are indented by `hang`, so wrapped list items line
    /// up under their own text rather than under the bullet.
    #[test]
    fn hanging_indent_applies_only_after_the_first_row() {
        let spans = vec![Span::raw("• alpha beta gamma")];
        let rows: Vec<String> = wrap_prose(&spans, 0, 2, 10).iter().map(text_of).collect();
        assert_eq!(rows, vec!["• alpha", "  beta", "  gamma"]);
    }

    fn prose(text: &str) -> RenderLine {
        RenderLine::Prose { spans: vec![Span::raw(text.to_string())], indent: 0, hang: 0 }
    }

    /// `block_rows` must report the row each block starts on, so a heading's
    /// block index can be turned into a page. A collapsed gap contributes no
    /// rows but still gets an entry, keeping the mapping one-to-one with the
    /// input blocks.
    #[test]
    fn block_rows_track_where_each_block_begins() {
        let blocks = vec![prose("a b"), RenderLine::Gap, prose("c")];
        let laid = layout(&blocks, 40, Spacing { line: 0, paragraph: 1 });
        // "a b" on row 0, one blank row for the paragraph gap, "c" on row 2.
        assert_eq!(laid.block_rows, vec![0, 1, 2]);
        assert_eq!(laid.text.lines.len(), 3);
    }

    /// A verbatim line wider than the column is clipped to a single row with a
    /// cut marker, never wrapped into a second row, so ASCII art keeps its
    /// shape. A line that fits is left exactly as it was.
    #[test]
    fn wide_verbatim_lines_are_clipped_not_wrapped() {
        let blocks = vec![RenderLine::Verbatim(Line::from(Span::raw("0123456789ABCDEF")))];
        let laid = layout(&blocks, 8, Spacing { line: 0, paragraph: 1 });

        assert_eq!(laid.text.lines.len(), 1, "a wide code line must stay one row");
        let rendered: String = laid.text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "0123456›", "clipped to width with a cut marker");
    }

    #[test]
    fn narrow_verbatim_lines_pass_through_unchanged() {
        let blocks = vec![RenderLine::Verbatim(Line::from(Span::raw("fits")))];
        let laid = layout(&blocks, 40, Spacing { line: 0, paragraph: 1 });
        let rendered: String = laid.text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "fits");
    }
}
