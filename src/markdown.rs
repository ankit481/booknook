//! Turning raw markdown text into styled, structured blocks.
//!
//! This module owns the pulldown-cmark parsing step. It reads a markdown
//! string and produces a list of `RenderLine`s, a flat sequence of blocks
//! that have not yet been wrapped to any particular width. The `wrap`
//! module takes that list and fits it to whatever column width the reader
//! is currently showing.

use pulldown_cmark::{CodeBlockKind, Event as MdEvent, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;

/// What kind of list the parser is currently inside. Ordered lists carry
/// the number of the next item, which is incremented as each `Item` is
/// emitted.
enum ListKind {
    Bullet,
    Ordered(u64),
}

/// A block of the parsed document, before it has been wrapped to any
/// particular width.
///
/// `Prose` is reflowable text. Word-wrap and line spacing get applied to it
/// later, in the `wrap` module. `Verbatim` is a finished terminal row that
/// must never be rewrapped, such as a line of code, since reflowing it
/// would break the code, or a language label. `Gap` is a break between two
/// blocks, left as an intention rather than a fixed number of blank rows,
/// because how much air a break gets is a layout decision that belongs to
/// the `wrap` module.
///
/// `Prose` carries two indents. `indent` applies to the block's first row,
/// and `hang` to every row after it. A bulleted list item, for example,
/// starts at its nesting indent and wraps to a deeper one, so that
/// continuation text lines up under the item's words rather than under its
/// bullet.
pub(crate) enum RenderLine {
    Prose { spans: Vec<Span<'static>>, indent: u16, hang: u16 },
    Verbatim(Line<'static>),
    Gap,
}

/// One entry in the document's table of contents.
///
/// `block` is the index into the parsed `blocks` where this heading's text
/// lands, which is what lets the sidebar's contents list jump straight to it.
/// A block index survives reflow: the row a heading sits on changes with the
/// column width, but which block it is does not, so the index is resolved to
/// a page only at draw time, against the current layout.
pub(crate) struct Heading {
    pub(crate) level: u8,
    pub(crate) text: String,
    pub(crate) block: usize,
}

/// The result of parsing a document: its blocks, ready to be wrapped, and a
/// flat table of contents drawn from its headings. Both come out of a single
/// pass, so the contents list can never drift out of step with the blocks it
/// points into.
pub(crate) struct Parsed {
    pub(crate) blocks: Vec<RenderLine>,
    pub(crate) headings: Vec<Heading>,
}

/// The leading marker for a quoted line, reused for every paragraph inside
/// a blockquote so that multi-paragraph quotes stay marked throughout.
fn quote_marker(theme: &Theme) -> Span<'static> {
    Span::styled("┃ ", Style::default().fg(theme.muted))
}

/// Turn a markdown string into a list of blocks, ready to be laid out at
/// whatever width the reading column ends up being.
///
/// pulldown-cmark hands over a stream of events, such as `Start(Heading)`,
/// `Text`, and `End(Heading)`. This function walks that stream, keeps a
/// stack of the styles currently in effect, and builds up blocks span by
/// span. `list_stack` and `quote_depth` track structural nesting, meaning
/// how deep the parser is inside lists or quotes, separately from `style`
/// and `style_stack`, which track inline formatting such as bold, italic,
/// and links. `indent` and `hang` track the leading indent, in columns,
/// for the current block's first row and for its continuation rows
/// respectively. A wrapped list item's second line, for example, should
/// line up under its text rather than under the bullet.
///
/// Colors are baked into the spans here, at parse time, which is why
/// switching themes re-runs this function over the document's raw text.
/// Documents are small and parsing is fast, so this is cheaper than
/// carrying a semantic role on every span and resolving it on every
/// frame.
pub(crate) fn render_markdown(input: &str, theme: &Theme) -> Parsed {
    let parser = Parser::new_ext(input, Options::all());

    let mut blocks: Vec<RenderLine> = Vec::new();
    let mut headings: Vec<Heading> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = Vec::new();
    let mut style = Style::default();
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut quote_depth = 0u32;
    let mut in_code_block = false;
    let mut indent = 0u16;
    let mut hang = 0u16;
    // The level of the heading currently being read, if any. Set when a
    // heading opens and taken back out when it closes, at which point the
    // text collected in `spans` is what the contents list should show.
    let mut heading_level: Option<u8> = None;

    for ev in parser {
        match ev {
            MdEvent::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    heading_level = Some(heading_level_to_u8(level));
                    style_stack.push(style);
                    style = style.fg(theme.heading).add_modifier(Modifier::BOLD);
                }
                Tag::Strong => {
                    style_stack.push(style);
                    style = style.add_modifier(Modifier::BOLD);
                }
                Tag::Emphasis => {
                    style_stack.push(style);
                    style = style.add_modifier(Modifier::ITALIC);
                }
                Tag::Link { .. } => {
                    style_stack.push(style);
                    style = style.fg(theme.link).add_modifier(Modifier::UNDERLINED);
                }
                Tag::BlockQuote(_) => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    quote_depth += 1;
                    style_stack.push(style);
                    style = style.fg(theme.quote).add_modifier(Modifier::ITALIC);
                }
                Tag::Paragraph if quote_depth > 0 => {
                    spans.push(quote_marker(theme));
                    // Continuation rows clear the "┃ " marker.
                    hang = 2;
                }
                Tag::CodeBlock(kind) => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    in_code_block = true;
                    if let CodeBlockKind::Fenced(lang) = &kind
                        && !lang.is_empty()
                    {
                        blocks.push(RenderLine::Verbatim(Line::from(Span::styled(
                            format!("  {lang}"),
                            Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC),
                        ))));
                    }
                }
                Tag::List(start) => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    list_stack.push(match start {
                        Some(n) => ListKind::Ordered(n),
                        None => ListKind::Bullet,
                    });
                }
                Tag::Item => {
                    let depth = list_stack.len();
                    // The nesting indent is kept out of the marker text and
                    // carried in `indent` instead. The word splitter throws
                    // leading whitespace away, so an indent baked into the
                    // marker string would simply vanish.
                    let nesting = 2 * depth.saturating_sub(1) as u16;
                    let marker = match list_stack.last_mut() {
                        Some(ListKind::Ordered(n)) => {
                            let marker = format!("{n}. ");
                            *n += 1;
                            marker
                        }
                        Some(ListKind::Bullet) => match depth {
                            1 => "• ".to_string(),
                            2 => "◦ ".to_string(),
                            _ => "▪ ".to_string(),
                        },
                        None => String::new(),
                    };
                    indent = nesting;
                    hang = nesting + UnicodeWidthStr::width(marker.as_str()) as u16;
                    spans.push(Span::styled(marker, Style::default().fg(theme.muted)));
                }
                _ => {}
            },
            MdEvent::End(tag) => match tag {
                TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::Item => {
                    // A heading's text is exactly the spans gathered since it
                    // opened. Record it, along with the index of the block
                    // that `flush_prose` is about to push, before the flush
                    // happens: at this moment `blocks.len()` is that block's
                    // future position.
                    if let TagEnd::Heading(_) = tag
                        && let Some(level) = heading_level.take()
                    {
                        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            headings.push(Heading { level, text, block: blocks.len() });
                        }
                    }
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    indent = 0;
                    hang = 0;
                    // List items sit tight against one another. Only real
                    // blocks earn a gap.
                    if !matches!(tag, TagEnd::Item) {
                        blocks.push(RenderLine::Gap);
                    }
                    if matches!(tag, TagEnd::Heading(_)) {
                        style = style_stack.pop().unwrap_or_default();
                    }
                }
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::Link => {
                    style = style_stack.pop().unwrap_or_default();
                }
                TagEnd::BlockQuote(_) => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    indent = 0;
                    hang = 0;
                    style = style_stack.pop().unwrap_or_default();
                    quote_depth = quote_depth.saturating_sub(1);
                    if quote_depth == 0 {
                        blocks.push(RenderLine::Gap);
                    }
                }
                TagEnd::CodeBlock => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    in_code_block = false;
                    blocks.push(RenderLine::Gap);
                }
                TagEnd::List(_) => {
                    flush_prose(&mut blocks, &mut spans, indent, hang);
                    list_stack.pop();
                    if list_stack.is_empty() {
                        blocks.push(RenderLine::Gap);
                    }
                }
                _ => {}
            },
            MdEvent::Text(text) => {
                if in_code_block {
                    // A fenced block's content arrives as one Text event
                    // with embedded newlines, plus a trailing newline
                    // before the closing fence. Only that last empty
                    // segment gets dropped, so real blank lines inside the
                    // code are preserved.
                    let mut code_lines = text.split('\n').peekable();
                    while let Some(line) = code_lines.next() {
                        if line.is_empty() && code_lines.peek().is_none() {
                            break;
                        }
                        blocks.push(RenderLine::Verbatim(Line::from(Span::styled(
                            format!("  {line}"),
                            Style::default().fg(theme.code),
                        ))));
                    }
                } else {
                    spans.push(Span::styled(text.into_string(), style));
                }
            }
            MdEvent::Code(text) => {
                spans.push(Span::styled(
                    text.into_string(),
                    style.fg(theme.code).add_modifier(Modifier::DIM),
                ));
            }
            MdEvent::SoftBreak => spans.push(Span::raw(" ")),
            MdEvent::HardBreak => flush_prose(&mut blocks, &mut spans, indent, hang),
            _ => {}
        }
    }
    flush_prose(&mut blocks, &mut spans, indent, hang);
    Parsed { blocks, headings }
}

/// pulldown-cmark models heading depth as an enum rather than a number. The
/// contents list wants a plain level to indent by, so this flattens it.
fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Move whatever spans have been collected into a finished prose block.
/// `mem::take` swaps `spans` for a fresh empty Vec and hands back the old
/// one to keep.
fn flush_prose(blocks: &mut Vec<RenderLine>, spans: &mut Vec<Span<'static>>, indent: u16, hang: u16) {
    if !spans.is_empty() {
        blocks.push(RenderLine::Prose { spans: std::mem::take(spans), indent, hang });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::THEMES;

    /// The text of a `Prose` block, joined across its spans and trimmed, or
    /// `None` if the block at that index is not prose.
    fn prose_text(blocks: &[RenderLine], index: usize) -> Option<String> {
        match blocks.get(index) {
            Some(RenderLine::Prose { spans, .. }) => {
                Some(spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim().to_string())
            }
            _ => None,
        }
    }

    /// Headings come out in document order, carry the right level, and each
    /// `block` index points at the prose block holding that heading's text.
    /// That last part is the contract the sidebar's jump-to-heading relies
    /// on: follow the index and you land on the heading itself.
    #[test]
    fn headings_record_level_text_and_their_block() {
        let input = "# Title\n\nSome body text.\n\n## A Subsection\n\nMore text.\n";
        let parsed = render_markdown(input, &THEMES[0]);

        assert_eq!(parsed.headings.len(), 2);
        assert_eq!(parsed.headings[0].level, 1);
        assert_eq!(parsed.headings[0].text, "Title");
        assert_eq!(parsed.headings[1].level, 2);
        assert_eq!(parsed.headings[1].text, "A Subsection");

        // The recorded block index must resolve back to the heading's text.
        for heading in &parsed.headings {
            assert_eq!(prose_text(&parsed.blocks, heading.block).as_deref(), Some(heading.text.as_str()));
        }
    }
}
