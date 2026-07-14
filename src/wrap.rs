//! Word-wrapping parsed blocks to a specific column width, and breaking
//! the result into pages a typesetter would sign off on.
//!
//! ratatui's own `Paragraph` widget can wrap text, but it gives no way to
//! see the individual wrapped rows it produces, which means there is no
//! way to insert a blank row between them for line spacing. This module
//! does the wrapping itself instead, so the reader can space every line
//! the way an e-reader's line-spacing setting would.
//!
//! Owning the rows also makes real page breaks possible. `layout` tags
//! each row with whether a page may end after it, and `paginate` walks
//! those rows page by page, ending a page a line or two early whenever
//! the arithmetic break would strand something: a heading at a page
//! bottom, a paragraph's first line left behind, its last line alone
//! overleaf. A short page is how books solve this too; the blank rows
//! that pad it out are invisible, while a bad break is not.

use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::markdown::{Heading, RenderLine, TableBlock};

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
    /// One entry per row of `text`: the break hints `paginate` reads.
    /// Empty once a `Laid` has been through pagination, since its rows are
    /// then final and carry nothing left to decide.
    pub(crate) meta: Vec<RowMeta>,
}

/// How one terminal row may relate to a page edge, decided while the row is
/// produced, when the block it belongs to is still at hand.
///
/// `keep_with_next` means a page must not end after this row: the first
/// line of a paragraph, any line of a heading, the rule under a table
/// header. `blank` marks structural air, gaps and line spacing, which a new
/// page sheds from its top; a book never opens a page with blank leading.
#[derive(Clone, Copy)]
pub(crate) struct RowMeta {
    pub(crate) keep_with_next: bool,
    pub(crate) blank: bool,
}

/// The rows of a document as they are produced, each paired with its break
/// hints. Blank rows inherit the keep flag of the row above them: a break
/// after trailing air reads, on the page, as a break after the content
/// itself, so whatever that content forbids its air must forbid too. This
/// inheritance is what carries a heading's keep-with-next across the gap
/// under it and onto the next paragraph without any special casing.
#[derive(Default)]
struct Rows {
    lines: Vec<Line<'static>>,
    meta: Vec<RowMeta>,
}

impl Rows {
    fn content(&mut self, line: Line<'static>, keep_with_next: bool) {
        self.lines.push(line);
        self.meta.push(RowMeta { keep_with_next, blank: false });
    }

    fn blank(&mut self) {
        let keep_with_next = self.meta.last().is_some_and(|m| m.keep_with_next);
        self.lines.push(Line::default());
        self.meta.push(RowMeta { keep_with_next, blank: true });
    }

    fn len(&self) -> usize {
        self.lines.len()
    }

    fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Lay out parsed blocks at a specific column width. Every prose block is
/// word-wrapped and spaced according to `spacing`. Verbatim blocks, such as
/// code and ASCII diagrams, keep their exact shape but are clipped to the
/// column so a wide one is cut cleanly at the edge rather than soft-wrapped
/// into a second row, which would break its alignment.
///
/// `headings` tells the row-tagging which prose blocks are headings, since
/// a heading must never be the last thing on a page. The result is not yet
/// page-shaped: hand it to `paginate` with a viewport height to get rows
/// whose page boundaries all fall at defensible breaks.
pub(crate) fn layout(blocks: &[RenderLine], headings: &[Heading], width: u16, spacing: Spacing) -> Laid {
    // Heading block indices arrive in document order, so membership is a
    // binary search rather than a set build per frame.
    let heading_blocks: Vec<usize> = headings.iter().map(|h| h.block).collect();

    let mut out = Rows::default();
    let mut block_rows: Vec<u16> = Vec::with_capacity(blocks.len());
    let mut prev_was_gap = false;

    for (index, block) in blocks.iter().enumerate() {
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
                        out.blank();
                    }
                }
                prev_was_gap = true;
                continue;
            }
            RenderLine::Verbatim(line) => out.content(clip_line(line, width as usize), false),
            RenderLine::Table(table) => layout_table(&mut out, table, width, spacing.line),
            RenderLine::Prose { spans, indent, hang } => {
                let is_heading = heading_blocks.binary_search(&index).is_ok();
                let wrapped = wrap_prose(spans, *indent, *hang, width);
                let count = wrapped.len();
                for (i, row) in wrapped.into_iter().enumerate() {
                    if i > 0 {
                        for _ in 0..spacing.line {
                            out.blank();
                        }
                    }
                    // A heading holds onto whatever follows it. Body prose
                    // guards its own edges: a break after the first row would
                    // orphan it, and a break after the second-to-last would
                    // widow the last. Two- and three-line paragraphs have no
                    // row that escapes both rules, so they never split at
                    // all, which is exactly what a typesetter would do.
                    let keep = if is_heading {
                        true
                    } else {
                        count > 1 && (i == 0 || i == count - 2)
                    };
                    out.content(row, keep);
                }
            }
        }
        prev_was_gap = false;
    }
    Laid { text: Text::from(out.lines), block_rows, meta: out.meta }
}

/// Break laid-out rows into pages of `viewport` rows whose boundaries all
/// land at acceptable spots, by ending a page early whenever the arithmetic
/// boundary falls somewhere `keep_with_next` forbids. The rows moved down
/// are replaced with blank padding, so every page except the last is
/// exactly `viewport` rows tall and the reader's page-scroll arithmetic
/// stays a plain multiplication.
///
/// Each new page also sheds structural blanks from its top, so no page
/// opens with the tail of the previous page's paragraph gap.
///
/// One run of glued rows can exceed a whole page, for instance a heading
/// atop a long paragraph in a very short window. There is no good break
/// inside such a run, so the page is filled to the brim and the break is
/// taken as it falls: a full page is the least bad option once every
/// option is bad.
pub(crate) fn paginate(laid: Laid, viewport: u16) -> Laid {
    let viewport = viewport.max(1) as usize;
    let mut lines = laid.text.lines;
    let meta = laid.meta;
    let total = lines.len();

    // Where each input row lands in the output, plus one entry past the end,
    // so block starts can be remapped after padding shifts everything.
    let mut map: Vec<usize> = vec![0; total + 1];
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut i = 0;

    while i < total {
        // A fresh page starts flush: leading air is dropped, and any block
        // that pointed into it resolves to the top of this page.
        while i < total && meta[i].blank {
            map[i] = out.len();
            i += 1;
        }
        if i >= total {
            break;
        }

        let page_start = out.len();
        let mut end = (i + viewport).min(total);
        if end < total {
            // The page is full with more to come: walk the break upward
            // until it sits after a row that allows one. Landing back at the
            // page's own start means the entire page is one glued run, and
            // the original, full-page break stands.
            let mut candidate = end;
            while candidate > i && meta[candidate - 1].keep_with_next {
                candidate -= 1;
            }
            if candidate > i {
                end = candidate;
            }
        }

        for row in i..end {
            map[row] = out.len();
            out.push(std::mem::take(&mut lines[row]));
        }
        i = end;

        // Pad a cut-short page out to the viewport, so the next page begins
        // on a boundary. The last page needs no padding; it just ends.
        if i < total {
            while (out.len() - page_start) % viewport != 0 {
                out.push(Line::default());
            }
        }
    }
    map[total] = out.len();

    let block_rows = laid
        .block_rows
        .iter()
        .map(|&row| map.get(row as usize).copied().unwrap_or(out.len()).min(u16::MAX as usize) as u16)
        .collect();
    Laid { text: Text::from(out), block_rows, meta: Vec::new() }
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

/// Blank columns between adjacent table columns.
const TABLE_GUTTER: usize = 2;

/// Lay a table out at the page's width: size the columns, then render the
/// header, a rule under it, and every row. Cells word-wrap inside their own
/// column, so a wide table gets taller rather than spilling off the page.
/// `line` blank rows separate one logical row from the next, matching the
/// line spacing prose gets, so the reader's spacing setting applies to
/// tables too.
///
/// Break hints: a page may end between logical rows but never inside one,
/// since half a wrapped cell is gibberish, and never right after the header
/// or its rule, which would strand the column names away from every value
/// they name.
fn layout_table(out: &mut Rows, table: &TableBlock, width: u16, line: u16) {
    let ncols = table
        .rows
        .iter()
        .map(Vec::len)
        .chain(std::iter::once(table.header.len()))
        .max()
        .unwrap_or(0);
    if ncols == 0 {
        return;
    }

    // A column's natural width is its widest cell, rendered on one line.
    let mut natural = vec![0usize; ncols];
    for row in std::iter::once(&table.header).chain(&table.rows) {
        for (col, cell) in row.iter().enumerate() {
            let cell_width: usize =
                cell.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
            natural[col] = natural[col].max(cell_width);
        }
    }

    let avail = (width as usize).saturating_sub(TABLE_GUTTER * (ncols - 1)).max(ncols);
    let widths = fit_columns(&natural, avail);

    if !table.header.is_empty() {
        // The header and its rule hold onto the first data row, so a page
        // never ends on column names with nothing under them.
        render_table_row(out, &table.header, &widths, true);
        let mut rule: Vec<Span<'static>> = Vec::new();
        for (col, col_width) in widths.iter().enumerate() {
            if col > 0 {
                rule.push(Span::raw(" ".repeat(TABLE_GUTTER)));
            }
            rule.push(Span::styled("─".repeat(*col_width), table.rule_style));
        }
        out.content(Line::from(rule), true);
    }
    for (i, row) in table.rows.iter().enumerate() {
        if i > 0 {
            for _ in 0..line {
                out.blank();
            }
        }
        render_table_row(out, row, &widths, false);
    }
}

/// Decide each column's width when the table must be squeezed. Columns that
/// fit inside an equal share of the available width keep their natural
/// size, and the space they leave unused is re-divided among the wider
/// ones, so a short id column never pays for a long description column.
fn fit_columns(natural: &[usize], avail: usize) -> Vec<usize> {
    if natural.iter().sum::<usize>() <= avail {
        return natural.to_vec();
    }
    let mut widths = vec![0usize; natural.len()];
    let mut pending: Vec<usize> = (0..natural.len()).collect();
    let mut left = avail;
    loop {
        let fair = (left / pending.len()).max(1);
        let fitting: Vec<usize> = pending.iter().copied().filter(|&i| natural[i] <= fair).collect();
        if fitting.is_empty() {
            // Every remaining column wants more than its share: split what
            // is left evenly, handing the first few one extra cell each so
            // no column is left short of the total.
            let n = pending.len();
            for (k, &i) in pending.iter().enumerate() {
                widths[i] = (left / n + usize::from(k < left % n)).max(1);
            }
            return widths;
        }
        for &i in &fitting {
            widths[i] = natural[i];
            left = left.saturating_sub(natural[i]);
        }
        pending.retain(|i| !fitting.contains(i));
        if pending.is_empty() {
            return widths;
        }
    }
}

/// Render one logical table row as however many terminal rows its tallest
/// cell needs. Each cell is word-wrapped to its column and padded out to
/// the column's width, so the next column starts at the same place on every
/// row. A cell that cannot wrap down to its column, such as one long
/// unbreakable word, is clipped the way a wide code line would be.
///
/// Every terminal row but the logical row's last is glued to the next, so a
/// page can never cut through the middle of a wrapped cell. `keep_all`
/// glues the last one too, for the header, which must never end a page.
fn render_table_row(out: &mut Rows, cells: &[Vec<Span<'static>>], widths: &[usize], keep_all: bool) {
    let empty: &[Span<'static>] = &[];
    let wrapped: Vec<Vec<Line<'static>>> = widths
        .iter()
        .enumerate()
        .map(|(col, col_width)| {
            let spans = cells.get(col).map(Vec::as_slice).unwrap_or(empty);
            wrap_prose(spans, 0, 0, *col_width as u16)
                .iter()
                .map(|row| clip_line(row, *col_width))
                .collect()
        })
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(0);
    for row_index in 0..height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (col, cell_rows) in wrapped.iter().enumerate() {
            if col > 0 {
                spans.push(Span::raw(" ".repeat(TABLE_GUTTER)));
            }
            let mut used = 0usize;
            if let Some(cell_row) = cell_rows.get(row_index) {
                used = cell_row.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
                spans.extend(cell_row.spans.iter().cloned());
            }
            // The last column carries no trailing padding; there is nothing
            // to its right to keep aligned.
            if col < widths.len() - 1 {
                spans.push(Span::raw(" ".repeat(widths[col].saturating_sub(used))));
            }
        }
        out.content(Line::from(spans), keep_all || row_index + 1 < height);
    }
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

    /// The visible text of every output row, in order, with blank rows shown
    /// as empty strings, so a whole pagination can be asserted at a glance.
    fn rows_of(laid: &Laid) -> Vec<String> {
        laid.text.lines.iter().map(text_of).collect()
    }

    const TIGHT: Spacing = Spacing { line: 0, paragraph: 1 };

    /// A heading whose section would start overleaf moves to the next page
    /// whole, taking at least the first lines of its paragraph with it. The
    /// page it leaves behind ends short, padded with blank rows so the next
    /// page still starts on an exact viewport boundary.
    #[test]
    fn a_heading_is_never_stranded_at_a_page_bottom() {
        // One row of filler, a gap, a heading, a gap, then a two-line
        // paragraph, which is atomic. At a viewport of 4, the arithmetic
        // break lands right after the heading's gap.
        let blocks = vec![prose("filler"), RenderLine::Gap, prose("Heading"), RenderLine::Gap, prose("alpha beta")];
        let headings = [Heading { level: 1, text: "Heading".into(), block: 2 }];
        let laid = paginate(layout(&blocks, &headings, 5, TIGHT), 4);

        assert_eq!(
            rows_of(&laid),
            vec![
                // Page one: the filler and a short page.
                "filler", "", "", "",
                // Page two: the heading with its whole paragraph.
                "Heading", "", "alpha", "beta",
            ]
        );
        // The heading's block entry must follow it to its new row.
        assert_eq!(laid.block_rows[2], 4);
    }

    /// A paragraph never leaves its first line at the bottom of a page. If
    /// only one line would fit, the whole paragraph waits for the next page.
    #[test]
    fn a_paragraph_first_line_is_never_orphaned() {
        // Two rows of filler (an atomic two-line paragraph), a gap, then a
        // four-line paragraph. At a viewport of 4 the arithmetic break falls
        // after the big paragraph's first line.
        let blocks = vec![prose("fill one"), RenderLine::Gap, prose("alpha beta gamma delta")];
        let laid = paginate(layout(&blocks, &[], 5, TIGHT), 4);

        assert_eq!(
            rows_of(&laid),
            vec![
                "fill", "one", "", "",
                "alpha", "beta", "gamma", "delta",
            ]
        );
    }

    /// A paragraph never sends its last line alone onto the next page: the
    /// break backs up one line so at least two travel together.
    #[test]
    fn a_paragraph_last_line_is_never_widowed() {
        // A four-line paragraph at a viewport of 3: the arithmetic break
        // would leave "delta" alone overleaf, so "gamma" goes with it.
        let blocks = vec![prose("alpha beta gamma delta")];
        let laid = paginate(layout(&blocks, &[], 5, TIGHT), 3);

        assert_eq!(rows_of(&laid), vec!["alpha", "beta", "", "gamma", "delta"]);
    }

    /// A gap that lands on a page boundary is simply consumed: the next page
    /// starts flush with real content, never with leftover blank rows, and no
    /// blank page is manufactured along the way.
    #[test]
    fn a_new_page_starts_flush_with_content() {
        let blocks = vec![prose("alpha"), RenderLine::Gap, prose("beta")];
        let laid = paginate(layout(&blocks, &[], 10, TIGHT), 1);

        assert_eq!(rows_of(&laid), vec!["alpha", "beta"]);
        // The gap block resolves to the top of the page it vanished into.
        assert_eq!(laid.block_rows, vec![0, 1, 1]);
    }

    /// A logical table row whose cells wrapped to two terminal rows crosses
    /// pages whole, and the header and rule travel with the first data row
    /// rather than ending a page as a title with nothing under it.
    #[test]
    fn a_table_never_splits_mid_cell_or_after_its_header() {
        let table = TableBlock {
            header: vec![cell("id"), cell("meaning")],
            rows: vec![vec![cell("a"), cell("alpha beta gamma")]],
            rule_style: Style::default(),
        };
        // Width 14 squeezes the second column to 10, wrapping the data row
        // onto two terminal rows. One filler row, then the table: at a
        // viewport of 4 the arithmetic break would cut between them.
        let blocks = vec![prose("filler"), RenderLine::Gap, RenderLine::Table(table)];
        let laid = paginate(layout(&blocks, &[], 14, TIGHT), 4);

        assert_eq!(
            rows_of(&laid),
            vec![
                "filler", "", "", "",
                "id  meaning", "──  ──────────", "a   alpha beta", "    gamma",
            ]
        );
    }

    /// A glued run taller than the page itself has no good break, so the page
    /// is filled to the brim instead of thrashing or leaving an empty page.
    #[test]
    fn a_run_taller_than_the_page_fills_it() {
        // A two-line heading? No: a heading followed by a long atomic
        // paragraph, at a viewport too small for both. Every row is glued,
        // so pagination must fall back to full pages.
        let blocks = vec![prose("Heading"), RenderLine::Gap, prose("alpha beta gamma")];
        let headings = [Heading { level: 1, text: "Heading".into(), block: 0 }];
        let laid = paginate(layout(&blocks, &headings, 5, TIGHT), 2);

        assert_eq!(rows_of(&laid), vec!["Heading", "", "alpha", "beta", "gamma"]);
    }

    /// `block_rows` must report the row each block starts on, so a heading's
    /// block index can be turned into a page. A collapsed gap contributes no
    /// rows but still gets an entry, keeping the mapping one-to-one with the
    /// input blocks.
    #[test]
    fn block_rows_track_where_each_block_begins() {
        let blocks = vec![prose("a b"), RenderLine::Gap, prose("c")];
        let laid = layout(&blocks, &[], 40, Spacing { line: 0, paragraph: 1 });
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
        let laid = layout(&blocks, &[], 8, Spacing { line: 0, paragraph: 1 });

        assert_eq!(laid.text.lines.len(), 1, "a wide code line must stay one row");
        let rendered: String = laid.text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "0123456›", "clipped to width with a cut marker");
    }

    fn cell(text: &str) -> Vec<Span<'static>> {
        vec![Span::raw(text.to_string())]
    }

    /// With room to spare, every column takes its natural width, cells pad
    /// out so columns start at the same place on every row, and the header
    /// gets a rule under it.
    #[test]
    fn table_columns_line_up_under_a_header_rule() {
        let table = TableBlock {
            header: vec![cell("Column"), cell("What it is")],
            rows: vec![
                vec![cell("id"), cell("Unique id")],
                vec![cell("title"), cell("Incident title")],
            ],
            rule_style: Style::default(),
        };
        let laid = layout(&[RenderLine::Table(table)], &[], 40, Spacing { line: 0, paragraph: 1 });
        let rows: Vec<String> = laid.text.lines.iter().map(text_of).collect();
        assert_eq!(
            rows,
            vec![
                "Column  What it is",
                "──────  ──────────────",
                "id      Unique id",
                "title   Incident title",
            ]
        );
    }

    /// A table wider than the page wraps cell text inside its column
    /// instead of spilling past the edge: the narrow column keeps its
    /// natural width, the wide one absorbs the squeeze and grows downward.
    #[test]
    fn wide_tables_wrap_cells_inside_their_columns() {
        let table = TableBlock {
            header: vec![cell("id"), cell("meaning")],
            rows: vec![vec![cell("a"), cell("alpha beta gamma")]],
            rule_style: Style::default(),
        };
        let laid = layout(&[RenderLine::Table(table)], &[], 14, Spacing { line: 0, paragraph: 1 });
        let rows: Vec<String> = laid.text.lines.iter().map(text_of).collect();
        assert_eq!(
            rows,
            vec![
                "id  meaning",
                "──  ──────────",
                "a   alpha beta",
                "    gamma",
            ]
        );
    }

    #[test]
    fn narrow_verbatim_lines_pass_through_unchanged() {
        let blocks = vec![RenderLine::Verbatim(Line::from(Span::raw("fits")))];
        let laid = layout(&blocks, &[], 40, Spacing { line: 0, paragraph: 1 });
        let rendered: String = laid.text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "fits");
    }
}
