use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use pulldown_cmark::{CodeBlockKind, Event as MdEvent, Options, Parser, Tag, TagEnd};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use unicode_width::UnicodeWidthStr;

/// A cool, bluish-black palette in the spirit of Tokyo Night / One Dark,
/// paired with crisp near-white body text -- the goal is IDE-dark-theme
/// backdrop, Kindle-crisp reading text.
mod theme {
    use ratatui::style::Color;

    pub const BG: Color = Color::Rgb(22, 23, 34);
    pub const FG: Color = Color::Rgb(214, 217, 226);
    pub const HEADING: Color = Color::Rgb(242, 243, 247);
    pub const CODE: Color = Color::Rgb(140, 170, 220);
    pub const MUTED: Color = Color::Rgb(94, 98, 122);
    pub const QUOTE: Color = Color::Rgb(158, 163, 184);
    pub const LINK: Color = Color::Rgb(122, 196, 178);
}

/// One entry in a directory listing.
struct Entry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

/// Which pane currently receives keyboard input. Both panes are always drawn
/// -- this only decides where `j`/`k`/arrows/Enter go.
enum Focus {
    Sidebar,
    Document,
}

/// Everything the running app needs to know.
///
/// Unlike the two-screen version of this app, the sidebar and the document
/// are no longer mutually exclusive -- both are always on screen, so their
/// state lives side by side in one `App` rather than behind an enum. `focus`
/// is the only thing that used to be an enum variant and still is, because
/// exactly one pane owns the keyboard at a time.
///
/// We deliberately store a *page number*, not a scroll row, for the reader.
/// An e-ink reader flips whole pages; it never scrolls mid-page. The row
/// offset for a given page is derived at draw time from the viewport height,
/// so `page` stays meaningful even if the terminal gets resized between
/// frames.
struct App {
    focus: Focus,
    dir: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    title: String,
    // The parsed document, *not yet wrapped to any particular width*. Layout
    // happens in `draw`, every frame, against the current column width -- see
    // `layout` for why that's the right place for it. Empty until a file has
    // been opened.
    blocks: Vec<RenderLine>,
    page: u16,
    // Whether the *last* draw showed two pages side by side. Set by
    // `draw_document` each frame (it's the only place that knows the current
    // width); read by `handle_document_key` to decide whether a page turn
    // should move by one page or by a whole spread.
    spread: bool,
    quit: bool,
}

fn main() -> Result<()> {
    let mut app = App {
        focus: Focus::Sidebar,
        dir: PathBuf::new(),
        entries: Vec::new(),
        selected: 0,
        title: String::new(),
        blocks: Vec::new(),
        page: 0,
        spread: false,
        quit: false,
    };

    // A CLI argument opens a file directly (sidebar starts in its folder), or
    // starts browsing in a directory; with neither, browse from the current
    // working directory.
    match std::env::args().nth(1).map(PathBuf::from) {
        Some(path) if path.is_dir() => enter_dir(&mut app, path),
        Some(path) => {
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
            enter_dir(&mut app, dir);
            load_file(&mut app, &path)?;
        }
        None => enter_dir(&mut app, std::env::current_dir()?),
    }

    // ratatui::init() enables raw mode + the alternate screen and installs a
    // panic hook that restores the terminal if we crash. restore() undoes it.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.quit {
        terminal.draw(|frame| draw(frame, app))?;
        handle_events(app)?;
    }
    Ok(())
}

/// Read `path`, parse it, and switch keyboard focus to the reader.
fn load_file(app: &mut App, path: &Path) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    app.title = path.display().to_string();
    app.blocks = render_markdown(&raw);
    app.page = 0;
    app.focus = Focus::Document;
    Ok(())
}

/// Point the sidebar at `dir` and list its contents.
fn enter_dir(app: &mut App, dir: PathBuf) {
    app.entries = list_dir(&dir);
    app.dir = dir;
    app.selected = 0;
}

/// List a directory's contents, folders first, both groups alphabetical --
/// the same convention NeoTree and most file explorers use. Entries we can't
/// read (permission errors, races) are silently skipped rather than failing
/// the whole listing.
fn list_dir(dir: &Path) -> Vec<Entry> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<Entry> = read_dir
        .filter_map(Result::ok)
        .map(|e| {
            let path = e.path();
            Entry {
                is_dir: path.is_dir(),
                name: e.file_name().to_string_lossy().into_owned(),
                path,
            }
        })
        .collect();
    entries.sort_by_key(|e| (!e.is_dir, e.name.to_lowercase()));
    entries
}

fn is_markdown(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("md") | Some("markdown"))
}

fn handle_events(app: &mut App) -> Result<()> {
    // read() blocks until the next terminal event (key, resize, mouse...).
    let Event::Key(key) = event::read()? else {
        return Ok(());
    };
    // On Windows, keys fire both a Press and a Release event. Ignore releases.
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    // Quit and focus-switching work from either pane, so they're handled
    // once here instead of duplicated in both key handlers.
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.quit = true;
            return Ok(());
        }
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Sidebar => Focus::Document,
                Focus::Document => Focus::Sidebar,
            };
            return Ok(());
        }
        _ => {}
    }

    match app.focus {
        Focus::Sidebar => handle_sidebar_key(app, key.code),
        Focus::Document => {
            handle_document_key(app, key.code);
            Ok(())
        }
    }
}

/// Move the selection, descend into a directory, go back up, or open a
/// markdown file. `→`/`l`/Enter mean "go deeper", `←`/`h`/Backspace mean "go
/// back" -- the same directions the reader uses for page turns.
fn handle_sidebar_key(app: &mut App, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.entries.is_empty() {
                app.selected = (app.selected + 1).min(app.entries.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => app.selected = app.selected.saturating_sub(1),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
            if let Some(parent) = app.dir.parent() {
                let parent = parent.to_path_buf();
                enter_dir(app, parent);
            }
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            if let Some(entry) = app.entries.get(app.selected) {
                if entry.is_dir {
                    let target = entry.path.clone();
                    enter_dir(app, target);
                } else if is_markdown(&entry.path) {
                    let target = entry.path.clone();
                    load_file(app, &target)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_document_key(app: &mut App, code: KeyCode) {
    // In a two-page spread, a "turn" flips the whole spread (2 pages), the
    // way it would with a real book -- not just the one page you're looking
    // at.
    let step = if app.spread { 2 } else { 1 };
    match code {
        KeyCode::Char(' ' | 'l' | 'j') | KeyCode::Right | KeyCode::Down | KeyCode::PageDown => {
            app.page = app.page.saturating_add(step);
        }
        KeyCode::Char('h' | 'k') | KeyCode::Left | KeyCode::Up | KeyCode::PageUp | KeyCode::Backspace => {
            app.page = app.page.saturating_sub(step);
        }
        KeyCode::Char('g') => app.page = 0,
        // We don't know the last page number until draw() computes it from
        // the viewport, so ask for "as far as possible" and let draw() clamp.
        KeyCode::Char('G') => app.page = u16::MAX,
        KeyCode::Char('o') => app.focus = Focus::Sidebar,
        _ => {}
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG)), frame.area());

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());
    let (body, status_bar) = (rows[0], rows[1]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(1)])
        .split(body);
    let (sidebar_area, doc_area) = (cols[0], cols[1]);

    draw_sidebar(frame, app, sidebar_area);
    let page_count = draw_document(frame, app, doc_area);
    draw_status_bar(frame, app, status_bar, page_count);
}

/// The width of one reading column, whether that's the single page or one
/// half of a two-page spread. Narrower than a typical 80-column terminal,
/// closer to what a book or e-reader uses.
const PAGE_WIDTH: u16 = 58;
/// Gap between the two pages of a spread, like a book's spine.
const GUTTER: u16 = 5;

fn draw_document(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    if app.blocks.is_empty() {
        app.spread = false;
        let hint = Paragraph::new(Line::from(Span::styled(
            "Select a markdown file to begin reading.",
            Style::default().fg(theme::MUTED).add_modifier(Modifier::ITALIC),
        )))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(theme::BG));
        frame.render_widget(hint, inset_vertical(area, area.height / 3, 0));
        return 1;
    }

    // Wide enough for two pages plus the gutter between them? Show a spread,
    // like an open book. Otherwise fall back to one page, like a phone or a
    // narrow window -- the same adaptive behavior e-reader apps use.
    app.spread = area.width >= PAGE_WIDTH * 2 + GUTTER;
    if app.spread {
        draw_spread(frame, app, area)
    } else {
        draw_single_page(frame, app, area)
    }
}

fn draw_single_page(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    // Generous margins so the text doesn't touch the pane's edge -- inset on
    // all four sides, not just top/bottom, so the page reads as a page and
    // not just a full-bleed column of text.
    let column = centered_column(area, PAGE_WIDTH);
    let reading_area = inset_horizontal(inset_vertical(column, 3, 2), 2, 2);

    // We wrap the document ourselves (see `layout`) so we can insert a blank
    // row between wrapped lines -- real line spacing, which Paragraph's own
    // wrapping has no hook for. Because this runs every frame against the
    // *current* width, resizing the terminal reflows correctly for free.
    let text = layout(&app.blocks, reading_area.width);
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(theme::FG).bg(theme::BG))
        .wrap(Wrap { trim: false });

    // line_count() tells us how many rows the text needs at this width. Our
    // own wrapping already fits `reading_area.width`, so this is normally
    // just `text.lines.len()` -- but it also catches the rare case of a
    // single word too long to fit (e.g. a long URL), which Paragraph's own
    // wrap still has to hard-break as a safety net.
    let viewport = reading_area.height.max(1);
    let total_rows = paragraph.line_count(reading_area.width) as u16;
    let page_count = total_rows.div_ceil(viewport).max(1);
    app.page = app.page.min(page_count - 1);

    let reader = paragraph.scroll((app.page * viewport, 0));
    frame.render_widget(reader, reading_area);
    page_count
}

/// Two pages side by side, like an open book. Both halves come from the same
/// wrapped text -- the left half shows `app.page`, the right shows the page
/// right after it -- so `app.page` always stays on an even number here, the
/// same way a real book's left-hand pages are always even-numbered.
fn draw_spread(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    let spread_width = (PAGE_WIDTH * 2 + GUTTER).min(area.width);
    let outer_margin = (area.width - spread_width) / 2;
    let spread = Rect { x: area.x + outer_margin, width: spread_width, ..area };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(PAGE_WIDTH), Constraint::Length(GUTTER), Constraint::Length(PAGE_WIDTH)])
        .split(spread);
    // Inset both pages the same amount on every side, including the edge
    // facing the spine -- otherwise the gutter's own width does all the work
    // of separating page from spine on one side but not the other, and the
    // two pages end up looking asymmetric.
    let left_area = inset_horizontal(inset_vertical(cols[0], 3, 2), 2, 2);
    let spine_area = cols[1];
    let right_area = inset_horizontal(inset_vertical(cols[2], 3, 2), 2, 2);

    let text = layout(&app.blocks, left_area.width);
    let viewport = left_area.height.max(1);
    let total_rows = text.lines.len() as u16;
    let page_count = total_rows.div_ceil(viewport).max(1);

    app.page = app.page.min(page_count.saturating_sub(1));
    if app.page % 2 == 1 {
        app.page -= 1;
    }

    let base_style = Style::default().fg(theme::FG).bg(theme::BG);
    let left = Paragraph::new(text.clone()).style(base_style).wrap(Wrap { trim: false }).scroll((app.page * viewport, 0));
    frame.render_widget(left, left_area);

    if app.page + 1 < page_count {
        let right_page = app.page + 1;
        let right = Paragraph::new(text).style(base_style).wrap(Wrap { trim: false }).scroll((right_page * viewport, 0));
        frame.render_widget(right, right_area);
    }

    // Centered in the gutter (GUTTER is 5 columns: 2 either side of the bar).
    let spine: Vec<Line<'static>> =
        (0..spine_area.height).map(|_| Line::from(Span::styled("  │  ", Style::default().fg(theme::MUTED)))).collect();
    frame.render_widget(Paragraph::new(Text::from(spine)).style(Style::default().bg(theme::BG)), spine_area);

    page_count
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let focused = matches!(app.focus, Focus::Sidebar);
    let border_color = if focused { theme::LINK } else { theme::MUTED };

    let label = app.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| app.dir.display().to_string());
    let block = Block::bordered()
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(format!(" {label} "), Style::default().fg(theme::HEADING).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // ".." is a hint, not a selectable row -- `h` / Backspace already go up a
    // directory, so it doesn't need its own slot in `selected`'s index space.
    let mut rows: Vec<Line<'static>> = Vec::new();
    if app.dir.parent().is_some() {
        rows.push(row_line("  ", "..", theme::MUTED, false));
    }
    for (i, entry) in app.entries.iter().enumerate() {
        let is_selected = i == app.selected;
        let name = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.clone() };
        let color = if entry.is_dir {
            theme::LINK
        } else if is_markdown(&entry.path) {
            theme::FG
        } else {
            theme::MUTED
        };
        rows.push(row_line(if is_selected { "› " } else { "  " }, &name, color, is_selected));
    }

    // Keep the selected row within view by scrolling just enough to bring it
    // onto the last visible line -- simple, and enough for a flat list.
    let visible = inner.height.max(1) as usize;
    let scroll = (app.selected + 1).saturating_sub(visible) as u16;

    let list = Paragraph::new(Text::from(rows))
        .style(Style::default().bg(theme::BG))
        .scroll((scroll, 0));
    frame.render_widget(list, inner);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, page_count: u16) {
    let text = match app.focus {
        Focus::Sidebar => "  ↑/↓ move · →/l open · ←/h up · Tab reader · q quit".to_string(),
        Focus::Document => {
            let position = if app.spread && app.page + 1 < page_count {
                format!("pages {}-{}", app.page + 1, app.page + 2)
            } else {
                format!("page {}", app.page + 1)
            };
            format!("  {}   {position} / {page_count}   ←/→ turn page · Tab/o sidebar · q quit", app.title)
        }
    };
    frame.render_widget(
        Paragraph::new(Line::from(text)).style(Style::default().fg(theme::MUTED).bg(theme::BG)),
        area,
    );
}

fn row_line(cursor: &str, label: &str, color: ratatui::style::Color, emphasize: bool) -> Line<'static> {
    let mut style = Style::default().fg(color);
    if emphasize {
        style = style.add_modifier(Modifier::BOLD);
    }
    Line::from(vec![
        Span::styled(cursor.to_string(), Style::default().fg(theme::MUTED)),
        Span::styled(label.to_string(), style),
    ])
}

/// Carve a fixed-width column out of the middle of `area`, capping the reading
/// width so lines stay short and easy on the eyes.
fn centered_column(area: Rect, max_width: u16) -> Rect {
    let width = area.width.min(max_width);
    let margin = (area.width - width) / 2;
    Rect {
        x: area.x + margin,
        y: area.y,
        width,
        height: area.height,
    }
}

/// Shrink a Rect's top and bottom edges, like a page margin.
fn inset_vertical(area: Rect, top: u16, bottom: u16) -> Rect {
    let shrink = top.saturating_add(bottom).min(area.height);
    Rect {
        y: area.y + top.min(area.height),
        height: area.height - shrink,
        ..area
    }
}

/// Shrink a Rect's left and right edges, like a page's side margins.
fn inset_horizontal(area: Rect, left: u16, right: u16) -> Rect {
    let shrink = left.saturating_add(right).min(area.width);
    Rect {
        x: area.x + left.min(area.width),
        width: area.width - shrink,
        ..area
    }
}

/// What kind of list we're currently inside. Ordered lists carry the number
/// of the *next* item, incremented as each `Item` is emitted.
enum ListKind {
    Bullet,
    Ordered(u64),
}

/// A block of the parsed document, *before* it's been wrapped to any
/// particular width. `Prose` is reflowable text -- word-wrap and line
/// spacing get applied to it at draw time. `Verbatim` is a finished terminal
/// row that must never be rewrapped: code lines (reflowing would break the
/// code), language labels, and the blank lines that separate blocks.
enum RenderLine {
    Prose { spans: Vec<Span<'static>>, hang: u16 },
    Verbatim(Line<'static>),
}

/// The leading marker for a quoted line, reused for every paragraph inside a
/// blockquote so multi-paragraph quotes stay marked throughout.
fn quote_marker() -> Span<'static> {
    Span::styled("┃ ", Style::default().fg(theme::MUTED))
}

/// Turn a markdown string into a list of blocks, ready to be laid out at
/// whatever width the reading column ends up being.
///
/// pulldown-cmark hands us a *stream* of events (Start/Text/End). We walk that
/// stream, keep a stack of the styles currently in effect, and build up
/// blocks span by span. `list_stack` and `quote_depth` track *structural*
/// nesting (how deep in lists/quotes we are) separately from
/// `style`/`style_stack`, which track *inline* formatting (bold/italic/link).
/// `hang` tracks the hanging indent (in columns) that continuation rows of
/// the *current* block should get -- e.g. a wrapped list item's second line
/// lines up under its text, not under the bullet.
fn render_markdown(input: &str) -> Vec<RenderLine> {
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
                    if let CodeBlockKind::Fenced(lang) = &kind {
                        if !lang.is_empty() {
                            blocks.push(RenderLine::Verbatim(Line::from(Span::styled(
                                format!("  {lang}"),
                                Style::default().fg(theme::MUTED).add_modifier(Modifier::ITALIC),
                            ))));
                        }
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
                    // A fenced block's content arrives as one Text event with
                    // embedded '\n's, plus a trailing newline before the
                    // closing fence -- drop only that last empty segment so
                    // real blank lines inside the code are preserved.
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

/// Move whatever spans we've collected into a finished prose block.
/// `mem::take` swaps `spans` for a fresh empty Vec and hands us back the old
/// one to keep.
fn flush_prose(blocks: &mut Vec<RenderLine>, spans: &mut Vec<Span<'static>>, hang: u16) {
    if !spans.is_empty() {
        blocks.push(RenderLine::Prose { spans: std::mem::take(spans), hang });
    }
}

/// Lay out parsed blocks at a specific column width: word-wrap every prose
/// block and insert a blank row between its wrapped lines (real line
/// spacing), while passing verbatim blocks (code, separators) through as-is.
fn layout(blocks: &[RenderLine], width: u16) -> Text<'static> {
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
/// each word's original style. Continuation rows (everything after the
/// first) are indented by `hang` columns, so a wrapped list item's second
/// line lands under its text instead of under the bullet.
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
