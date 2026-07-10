//! Turning raw markdown text into styled, structured blocks.
//!
//! This module owns the pulldown-cmark parsing step. It reads a markdown
//! string and produces a list of `RenderLine`s, a flat sequence of blocks
//! that have not yet been wrapped to any particular width. The `wrap`
//! module takes that list and fits it to whatever column width the reader
//! is currently showing.

use pulldown_cmark::{CodeBlockKind, Event as MdEvent, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::theme;

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
/// must never be rewrapped: a code line, since reflowing it would break the
/// code, a language label, or one of the blank lines that separate blocks.
pub(crate) enum RenderLine {
    Prose { spans: Vec<Span<'static>>, hang: u16 },
    Verbatim(Line<'static>),
}

/// The leading marker for a quoted line, reused for every paragraph inside
/// a blockquote so that multi-paragraph quotes stay marked throughout.
fn quote_marker() -> Span<'static> {
    Span::styled("┃ ", Style::default().fg(theme::MUTED))
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
/// and links. `hang` tracks the hanging indent, in columns, that
/// continuation rows of the current block should get. A wrapped list
/// item's second line, for example, should line up under its text rather
/// than under the bullet.
pub(crate) fn render_markdown(input: &str) -> Vec<RenderLine> {
    let parser = Parser::new_ext(input, Options::all());

    let mut blocks: Vec<RenderLine> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = Vec::new();
    let mut style = Style::default();
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut quote_depth = 0u32;
    let mut in_code_block = false;
    let mut hang = 0u16;

    for ev in parser {
        match ev {
            MdEvent::Start(tag) => match tag {
                Tag::Heading { .. } => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    style_stack.push(style);
                    style = style.fg(theme::HEADING).add_modifier(Modifier::BOLD);
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
                    style = style.fg(theme::LINK).add_modifier(Modifier::UNDERLINED);
                }
                Tag::BlockQuote(_) => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    quote_depth += 1;
                    style_stack.push(style);
                    style = style.fg(theme::QUOTE).add_modifier(Modifier::ITALIC);
                }
                Tag::Paragraph if quote_depth > 0 => {
                    spans.push(quote_marker());
                    hang = 2;
                }
                Tag::CodeBlock(kind) => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    in_code_block = true;
                    if let CodeBlockKind::Fenced(lang) = &kind
                        && !lang.is_empty()
                    {
                        blocks.push(RenderLine::Verbatim(Line::from(Span::styled(
                            format!("  {lang}"),
                            Style::default().fg(theme::MUTED).add_modifier(Modifier::ITALIC),
                        ))));
                    }
                }
                Tag::List(start) => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    list_stack.push(match start {
                        Some(n) => ListKind::Ordered(n),
                        None => ListKind::Bullet,
                    });
                }
                Tag::Item => {
                    let depth = list_stack.len();
                    let indent = "  ".repeat(depth.saturating_sub(1));
                    let marker = match list_stack.last_mut() {
                        Some(ListKind::Ordered(n)) => {
                            let marker = format!("{indent}{n}. ");
                            *n += 1;
                            marker
                        }
                        Some(ListKind::Bullet) => {
                            let bullet = match depth {
                                1 => "• ",
                                2 => "◦ ",
                                _ => "▪ ",
                            };
                            format!("{indent}{bullet}")
                        }
                        None => String::new(),
                    };
                    hang = UnicodeWidthStr::width(marker.as_str()) as u16;
                    spans.push(Span::styled(marker, Style::default().fg(theme::MUTED)));
                }
                _ => {}
            },
            MdEvent::End(tag) => match tag {
                TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::Item => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    hang = 0;
                    if !matches!(tag, TagEnd::Item) {
                        blocks.push(RenderLine::Verbatim(Line::default())); // gap between blocks
                    }
                    if matches!(tag, TagEnd::Heading(_)) {
                        style = style_stack.pop().unwrap_or_default();
                    }
                }
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::Link => {
                    style = style_stack.pop().unwrap_or_default();
                }
                TagEnd::BlockQuote(_) => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    hang = 0;
                    style = style_stack.pop().unwrap_or_default();
                    quote_depth = quote_depth.saturating_sub(1);
                    if quote_depth == 0 {
                        blocks.push(RenderLine::Verbatim(Line::default()));
                    }
                }
                TagEnd::CodeBlock => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    in_code_block = false;
                    blocks.push(RenderLine::Verbatim(Line::default()));
                }
                TagEnd::List(_) => {
                    flush_prose(&mut blocks, &mut spans, hang);
                    list_stack.pop();
                    if list_stack.is_empty() {
                        blocks.push(RenderLine::Verbatim(Line::default()));
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
                            Style::default().fg(theme::CODE),
                        ))));
                    }
                } else {
                    spans.push(Span::styled(text.into_string(), style));
                }
            }
            MdEvent::Code(text) => {
                spans.push(Span::styled(
                    text.into_string(),
                    style.fg(theme::CODE).add_modifier(Modifier::DIM),
                ));
            }
            MdEvent::SoftBreak => spans.push(Span::raw(" ")),
            MdEvent::HardBreak => flush_prose(&mut blocks, &mut spans, hang),
            _ => {}
        }
    }
    flush_prose(&mut blocks, &mut spans, hang);
    blocks
}

/// Move whatever spans have been collected into a finished prose block.
/// `mem::take` swaps `spans` for a fresh empty Vec and hands back the old
/// one to keep.
fn flush_prose(blocks: &mut Vec<RenderLine>, spans: &mut Vec<Span<'static>>, hang: u16) {
    if !spans.is_empty() {
        blocks.push(RenderLine::Prose { spans: std::mem::take(spans), hang });
    }
}
