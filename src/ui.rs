//! Rendering the application state to the terminal.
//!
//! Every function here takes the current `App` and a ratatui `Frame`, and
//! draws. A couple of functions take `&mut App`, because they need to
//! clamp a value like the current page once the true page count is known,
//! but nothing here changes application state beyond that kind of
//! bookkeeping. Turning input into state changes is the `events` module's
//! job.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::browser::is_readable;
use crate::theme::Theme;
use crate::wrap;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = app.theme();
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), frame.area());

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());
    let (body, status_bar) = (rows[0], rows[1]);

    // The sidebar recedes while the reader has focus, giving the page the
    // whole window, and returns when Tab or `o` hands the keyboard back to
    // it. Zero-width is how it recedes: the same layout runs either way, so
    // there is exactly one split to keep consistent with `document_area`.
    let sidebar_width = if app.sidebar_visible() { SIDEBAR_WIDTH } else { 0 };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
        .split(body);
    let (sidebar_area, doc_area) = (cols[0], cols[1]);

    // The document is drawn first on purpose. Laying it out is what reveals
    // which heading the current page falls under, and the sidebar's contents
    // list wants that answer to highlight the right entry. Drawing the reader
    // first means the sidebar sees this frame's value, not last frame's.
    let page_count = draw_document(frame, app, doc_area);
    if sidebar_width > 0 {
        draw_sidebar(frame, app, sidebar_area);
    }
    draw_status_bar(frame, app, status_bar, page_count);
}

/// The gap between the two pages of a spread, like a book's spine.
const GUTTER: u16 = 5;

/// How wide the left sidebar is. Wide enough for the file browser and for the
/// contents list to show most headings on a single line before wrapping.
pub(crate) const SIDEBAR_WIDTH: u16 = 34;

/// The rect the document is drawn into, to the right of the sidebar and above
/// the status bar. This is the same region `draw` lays the reader out in, and
/// the page-turn animation clips its slide to it so only the reading column
/// moves. Kept as a plain function so the layout math has one home: the
/// vertical split reserves the bottom row for the status bar, and the
/// horizontal one reserves the left columns for the sidebar, when it is
/// showing. `sidebar` must be the same `App::sidebar_visible` answer the
/// frame was drawn with, or the two layouts drift apart.
pub(crate) fn document_area(area: Rect, sidebar: bool) -> Rect {
    let sidebar_width = if sidebar { SIDEBAR_WIDTH } else { 0 };
    Rect {
        x: area.x + sidebar_width.min(area.width),
        y: area.y,
        width: area.width.saturating_sub(sidebar_width),
        height: area.height.saturating_sub(1),
    }
}

fn draw_document(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    let theme = app.theme();
    if app.blocks.is_empty() {
        app.spread = false;
        app.active_heading = None;
        let hint = Paragraph::new(Line::from(Span::styled(
            "Select a markdown file to begin reading.",
            Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center)
        .style(Style::default().bg(theme.bg));
        frame.render_widget(hint, inset_vertical(area, area.height / 3, 0));
        return 1;
    }

    // Is there room for two pages plus the gutter between them? Then show
    // a spread, like an open book. Otherwise fall back to one page, like a
    // phone or a narrow window would. This is the same kind of adaptive
    // behavior e-reader apps use.
    app.spread = area.width >= app.page_width * 2 + GUTTER;
    if app.spread {
        draw_spread(frame, app, area)
    } else {
        draw_single_page(frame, app, area)
    }
}

fn draw_single_page(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    let theme = app.theme();
    // Generous margins keep the text off the pane's edge. The inset
    // applies on all four sides, not just top and bottom, so the page
    // reads as a page and not just a full-bleed column of text.
    let column = centered_column(area, app.page_width);
    let reading_area = inset_horizontal(inset_vertical(column, 3, 2), 2, 2);

    // The column, margins included, is painted in the page shade, so the
    // text sits on a sheet rather than on the terminal background.
    frame.render_widget(Block::default().style(Style::default().bg(theme.page)), column);

    // The document is wrapped by the `wrap` module rather than left to
    // Paragraph's own wrapping, so that a blank row can be inserted
    // between wrapped lines for real line spacing, and then paginated so
    // every page boundary falls at a break a typesetter would allow.
    // Because this runs every frame against the current width and height,
    // resizing the terminal reflows and re-breaks the pages for free.
    let viewport = reading_area.height.max(1);
    let laid = wrap::paginate(wrap::layout(&app.blocks, &app.headings, reading_area.width, app.spacing), viewport);
    let total_rows = laid.text.lines.len() as u16;
    let page_count = total_rows.div_ceil(viewport).max(1);
    let paragraph = Paragraph::new(laid.text)
        .style(Style::default().fg(theme.fg).bg(theme.page))
        .wrap(Wrap { trim: false });

    apply_pending_jump(app, &laid.block_rows, viewport);
    app.page = app.page.min(page_count - 1);
    update_active_heading(app, &laid.block_rows, viewport);

    let reader = paragraph.scroll((app.page * viewport, 0));
    frame.render_widget(reader, reading_area);
    page_count
}

/// Two pages side by side, like an open book. Both halves come from the
/// same wrapped text. The left half shows `app.page`, and the right shows
/// the page right after it, so `app.page` always stays on an even number
/// here, the same way a real book's left-hand pages are always
/// even-numbered.
fn draw_spread(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    let theme = app.theme();
    let spread_width = (app.page_width * 2 + GUTTER).min(area.width);
    let outer_margin = (area.width - spread_width) / 2;
    let spread = Rect { x: area.x + outer_margin, width: spread_width, ..area };

    // The whole spread, gutter included, is one sheet of paper. The spine
    // is a line drawn on it, not a gap between two separate sheets.
    frame.render_widget(Block::default().style(Style::default().bg(theme.page)), spread);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(app.page_width),
            Constraint::Length(GUTTER),
            Constraint::Length(app.page_width),
        ])
        .split(spread);
    // Both pages get the same inset on every side, including the edge
    // facing the spine. Otherwise the gutter's own width would do all the
    // work of separating page from spine on one side but not the other,
    // and the two pages would end up looking asymmetric.
    let left_area = inset_horizontal(inset_vertical(cols[0], 3, 2), 2, 2);
    let spine_area = cols[1];
    let right_area = inset_horizontal(inset_vertical(cols[2], 3, 2), 2, 2);

    let viewport = left_area.height.max(1);
    let laid = wrap::paginate(wrap::layout(&app.blocks, &app.headings, left_area.width, app.spacing), viewport);
    let text = laid.text;
    let total_rows = text.lines.len() as u16;
    let page_count = total_rows.div_ceil(viewport).max(1);

    apply_pending_jump(app, &laid.block_rows, viewport);
    app.page = app.page.min(page_count.saturating_sub(1));
    if app.page % 2 == 1 {
        app.page -= 1;
    }
    update_active_heading(app, &laid.block_rows, viewport);

    let base_style = Style::default().fg(theme.fg).bg(theme.page);
    let left = Paragraph::new(text.clone()).style(base_style).wrap(Wrap { trim: false }).scroll((app.page * viewport, 0));
    frame.render_widget(left, left_area);

    if app.page + 1 < page_count {
        let right_page = app.page + 1;
        let right = Paragraph::new(text).style(base_style).wrap(Wrap { trim: false }).scroll((right_page * viewport, 0));
        frame.render_widget(right, right_area);
    }

    // The bar sits centered in the gutter. GUTTER is 5 columns wide, two on
    // either side of the bar itself.
    let spine: Vec<Line<'static>> =
        (0..spine_area.height).map(|_| Line::from(Span::styled("  │  ", Style::default().fg(theme.muted)))).collect();
    frame.render_widget(Paragraph::new(Text::from(spine)).style(Style::default().bg(theme.page)), spine_area);

    page_count
}

/// A jump requested from the contents list, resolved to a page. The event
/// handler only knows the target block; the row that block lands on, and so
/// the page it belongs to, is not known until layout has run at the current
/// width. This is where that becomes a real page number, after which the
/// request is cleared so it fires exactly once.
fn apply_pending_jump(app: &mut App, block_rows: &[u16], viewport: u16) {
    if let Some(block) = app.pending_jump.take() {
        let row = block_rows.get(block).copied().unwrap_or(0);
        app.page = row / viewport;
    }
}

/// Work out which heading the page on screen falls under, and record it so
/// the contents list can highlight that entry. The active heading is the last
/// one whose first row has already scrolled into or above the visible
/// window. Headings and their rows both run in document order, so the search
/// can stop at the first heading that has not appeared yet.
fn update_active_heading(app: &mut App, block_rows: &[u16], viewport: u16) {
    if app.headings.is_empty() {
        app.active_heading = None;
        return;
    }
    // A spread shows two pages at once, so the right-hand page counts as
    // visible too when deciding what the reader is currently looking at.
    let pages_shown = if app.spread { 2 } else { 1 };
    let bottom = (app.page as u32 + pages_shown) * viewport as u32;

    let mut active = None;
    for (i, heading) in app.headings.iter().enumerate() {
        let row = block_rows.get(heading.block).copied().unwrap_or(0) as u32;
        if row < bottom {
            active = Some(i);
        } else {
            break;
        }
    }
    app.active_heading = active;
}

/// The sidebar holds two stacked lists: the file browser on top, and the
/// open document's table of contents below it. The contents list only
/// appears once there is a document with headings to show; until then the
/// file browser has the whole pane to itself.
fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    if app.headings.is_empty() {
        draw_files(frame, app, area);
        return;
    }

    // The contents list is sized to the rows it actually needs once headings
    // are wrapped, so a document whose titles wrap gets a taller box rather
    // than a scrollbar it did not need. It is capped at roughly two thirds of
    // the pane so the file browser is never crowded out entirely, and the
    // browser takes whatever is left, with a floor so it never vanishes.
    let inner_width = area.width.saturating_sub(2);
    let needed = toc_row_count(app, inner_width) as u16 + 2;
    let cap = (area.height * 2 / 3).max(3);
    let toc_height = needed.min(cap).max(3);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(toc_height)])
        .split(area);
    draw_files(frame, app, rows[0]);
    draw_toc(frame, app, rows[1]);
}

fn draw_files(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let focused = matches!(app.focus, Focus::Files);
    let border_color = if focused { theme.link } else { theme.muted };

    let label = app.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| app.dir.display().to_string());
    let block = Block::bordered()
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(format!(" {label} "), Style::default().fg(theme.heading).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // ".." is a hint, not a selectable row. `h` and Backspace already go up
    // a directory, so it does not need its own slot in `selected`'s index
    // space.
    let mut rows: Vec<Line<'static>> = Vec::new();
    if app.dir.parent().is_some() {
        rows.push(row_line(theme, "  ", "..", theme.muted, false));
    }
    for (i, entry) in app.entries.iter().enumerate() {
        let is_selected = focused && i == app.selected;
        let name = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.clone() };
        let color = if entry.is_dir {
            theme.link
        } else if is_readable(&entry.path) {
            theme.fg
        } else {
            theme.muted
        };
        rows.push(row_line(theme, if is_selected { "› " } else { "  " }, &name, color, is_selected));
    }

    // The selected row is kept within view by scrolling just enough to
    // bring it onto the last visible line. This is a simple approach, and
    // it is enough for a flat list.
    let visible = inner.height.max(1) as usize;
    let scroll = (app.selected + 1).saturating_sub(visible) as u16;

    let list = Paragraph::new(Text::from(rows))
        .style(Style::default().bg(theme.bg))
        .scroll((scroll, 0));
    frame.render_widget(list, inner);
}

/// The open document's headings, as a jump list. The entry the reader is
/// currently under is drawn in the heading color, so the contents list
/// doubles as a "you are here" marker, not just a way to navigate. When the
/// pane has focus, a cursor marks the entry a jump would land on.
///
/// A heading too long for the pane wraps onto continuation lines rather than
/// being clipped at the edge, so no title is ever cut off mid-word.
fn draw_toc(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let focused = matches!(app.focus, Focus::Toc);
    let border_color = if focused { theme.link } else { theme.muted };

    let block = Block::bordered()
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" Contents ", Style::default().fg(theme.heading).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let width = inner.width as usize;
    let mut rows: Vec<Line<'static>> = Vec::new();
    // Where each heading's first rendered row begins, so the cursor can be
    // scrolled into view once wrapping makes the row count exceed the
    // heading count.
    let mut heading_starts: Vec<usize> = Vec::with_capacity(app.headings.len());

    for (i, heading) in app.headings.iter().enumerate() {
        heading_starts.push(rows.len());

        let is_selected = focused && i == app.toc_selected;
        let is_active = app.active_heading == Some(i);
        let color = if is_selected {
            theme.fg
        } else if is_active {
            theme.heading
        } else {
            theme.muted
        };
        let mut style = Style::default().fg(color);
        if is_selected || is_active {
            style = style.add_modifier(Modifier::BOLD);
        }

        // Deeper headings sit further in, so the list reads as an outline
        // rather than a flat run of titles. The text wraps into whatever
        // room is left after the two-column cursor and that indent.
        let indent = 2 * heading.level.saturating_sub(1) as usize;
        let text_width = width.saturating_sub(2 + indent).max(1);
        for (row, segment) in wrap_text(&heading.text, text_width).into_iter().enumerate() {
            // Only the first row carries the cursor; continuation rows align
            // under the heading's own text.
            let cursor = if row == 0 && is_selected { "› " } else { "  " };
            rows.push(Line::from(vec![
                Span::styled(cursor.to_string(), Style::default().fg(theme.muted)),
                Span::styled(format!("{}{segment}", " ".repeat(indent)), style),
            ]));
        }
    }

    // Keep the selected heading in view: scroll just enough to bring its last
    // row onto the pane, but never past its first row, so a heading taller
    // than the pane still shows its start.
    let visible = inner.height.max(1) as usize;
    let first = heading_starts.get(app.toc_selected).copied().unwrap_or(0);
    let next = heading_starts.get(app.toc_selected + 1).copied().unwrap_or(rows.len());
    let last = next.saturating_sub(1);
    let scroll = last.saturating_sub(visible.saturating_sub(1)).min(first) as u16;

    let list = Paragraph::new(Text::from(rows))
        .style(Style::default().bg(theme.bg))
        .scroll((scroll, 0));
    frame.render_widget(list, inner);
}

/// The number of rows the contents list needs at a given inner width, once
/// every heading is wrapped. Used to size the contents box so it grows to fit
/// its content rather than clipping it.
fn toc_row_count(app: &App, inner_width: u16) -> usize {
    let width = inner_width as usize;
    app.headings
        .iter()
        .map(|heading| {
            let indent = 2 * heading.level.saturating_sub(1) as usize;
            let text_width = width.saturating_sub(2 + indent).max(1);
            wrap_text(&heading.text, text_width).len()
        })
        .sum()
}

/// Greedy word wrap of a plain string to `width` columns. A word longer than
/// the whole width is left on its own line rather than split, since a clipped
/// long word in a heading is rare and less jarring than a hard break through
/// the middle of one. Always returns at least one line.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let needed = if current_len == 0 { word_len } else { current_len + 1 + word_len };
        if needed > width && current_len > 0 {
            lines.push(std::mem::take(&mut current));
            current_len = 0;
        }
        if current_len > 0 {
            current.push(' ');
            current_len += 1;
        }
        current.push_str(word);
        current_len += word_len;
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, page_count: u16) {
    let theme = app.theme();
    let text = match app.focus {
        Focus::Files => {
            let next = if app.headings.is_empty() { "reader" } else { "contents" };
            format!("  ↑/↓ move · →/l open · ←/h up · Tab {next} · t {} · q quit", theme.name)
        }
        Focus::Toc => {
            format!("  ↑/↓ move · →/l/Enter jump · ←/h files · Tab reader · t {} · q quit", theme.name)
        }
        Focus::Document => {
            let position = if app.spread && app.page + 1 < page_count {
                format!("pages {}-{}", app.page + 1, app.page + 2)
            } else {
                format!("page {}", app.page + 1)
            };
            let flip = if app.animate { " · flip on" } else { "" };
            format!(
                "  {position} / {page_count}   line {} · para {} · width {} · {}{flip}   [ ] {{ }} -/+ t · a · Tab sidebar · q",
                app.spacing.line, app.spacing.paragraph, app.page_width, theme.name
            )
        }
    };
    frame.render_widget(
        Paragraph::new(Line::from(text)).style(Style::default().fg(theme.muted).bg(theme.bg)),
        area,
    );
}

fn row_line(theme: &Theme, cursor: &str, label: &str, color: Color, emphasize: bool) -> Line<'static> {
    let mut style = Style::default().fg(color);
    if emphasize {
        style = style.add_modifier(Modifier::BOLD);
    }
    Line::from(vec![
        Span::styled(cursor.to_string(), Style::default().fg(theme.muted)),
        Span::styled(label.to_string(), style),
    ])
}

/// Carve a fixed-width column out of the middle of `area`, capping the
/// reading width so lines stay short and easy on the eyes.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::markdown;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Flatten a rendered frame's cells into one string. Text within a single
    /// row stays contiguous, which is all the assertions below need.
    fn screen(terminal: &Terminal<TestBackend>) -> String {
        terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect()
    }

    /// A full frame render, headless, exercising the real draw path: the
    /// contents box appears in the sidebar and lists the document's headings,
    /// and the reader shows the body. This is the closest thing to launching
    /// the app that runs without a terminal.
    #[test]
    fn renders_contents_list_alongside_the_reader() {
        let mut app = App::new();
        let doc = "# Chapter One\n\nSome body text here.\n\n## A Section\n\nMore text.\n";
        let parsed = markdown::render_markdown(doc, app.theme());
        app.blocks = parsed.blocks;
        app.headings = parsed.headings;
        app.title = "test.md".into();

        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let text = screen(&terminal);
        assert!(text.contains("Contents"), "sidebar should show the contents box:\n{text}");
        assert!(text.contains("Chapter One"), "the H1 should appear:\n{text}");
        assert!(text.contains("A Section"), "the H2 should appear:\n{text}");
        assert!(text.contains("Some body text"), "the reader should show the body:\n{text}");
    }

    /// Selecting a heading in the contents list turns the page to it. The
    /// event handler only records the target block; this confirms the draw
    /// step resolves that block to a real page and clears the request. A very
    /// short terminal forces a tiny viewport, so the second heading genuinely
    /// lands past the first page.
    #[test]
    fn a_pending_jump_turns_to_the_heading_page() {
        let mut app = App::new();
        let doc = "# Chapter One\n\nSome body text here.\n\n## A Section\n\nMore text.\n";
        let parsed = markdown::render_markdown(doc, app.theme());
        app.blocks = parsed.blocks;
        app.headings = parsed.headings;
        // Ask to jump to the second heading, as choosing it in the sidebar would.
        app.pending_jump = Some(app.headings[1].block);

        let mut terminal = Terminal::new(TestBackend::new(100, 8)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        assert!(app.page >= 1, "the jump should leave the first page, landed on {}", app.page);
        assert_eq!(app.pending_jump, None, "the jump request should be consumed");
        assert_eq!(app.active_heading, Some(1), "the second heading should now be the active one");
    }

    /// The standalone `document_area` used by the page-turn animation must
    /// carve out the exact rect the real draw path hands the reader, so the
    /// slide lines up with the column it is sliding. This pins the two together
    /// at a representative terminal size, both with the sidebar showing and
    /// with it receded.
    #[test]
    fn document_area_matches_the_drawn_layout() {
        let area = Rect::new(0, 0, 100, 30);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        for sidebar in [true, false] {
            let width = if sidebar { SIDEBAR_WIDTH } else { 0 };
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(width), Constraint::Min(1)])
                .split(rows[0]);
            assert_eq!(document_area(area, sidebar), cols[1]);
        }
    }

    /// While the reader has focus the sidebar recedes entirely: no file
    /// list, no contents box, just the page. Handing focus back to the
    /// sidebar brings it back. The page itself must stay put through both
    /// frames, since the reading column's width never changed.
    #[test]
    fn the_sidebar_recedes_while_reading_and_returns_on_focus() {
        let mut app = App::new();
        let doc = "# Chapter One\n\nSome body text here.\n";
        let parsed = markdown::render_markdown(doc, app.theme());
        app.blocks = parsed.blocks;
        app.headings = parsed.headings;
        app.focus = Focus::Document;

        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let reading = screen(&terminal);
        assert!(!reading.contains("Contents"), "the contents box should recede while reading:\n{reading}");
        assert!(reading.contains("Some body text"), "the page itself must remain:\n{reading}");

        app.focus = Focus::Files;
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let browsing = screen(&terminal);
        assert!(browsing.contains("Contents"), "the sidebar should return with focus:\n{browsing}");
    }

    /// With nothing open there is nothing to read, so the sidebar holds its
    /// ground even if focus lands on the empty reader; receding would leave
    /// a blank screen with no way to see where you are.
    #[test]
    fn an_empty_reader_keeps_the_sidebar() {
        let mut app = App::new();
        app.focus = Focus::Document;
        assert!(app.sidebar_visible(), "no document means the sidebar stays");
    }

    #[test]
    fn wrap_text_packs_words_and_never_splits_them() {
        assert_eq!(wrap_text("alpha beta gamma", 11), vec!["alpha beta", "gamma"]);
        // A single word wider than the column is left whole rather than cut.
        assert_eq!(wrap_text("supercalifragilistic", 8), vec!["supercalifragilistic"]);
        assert_eq!(wrap_text("", 10), vec![""]);
    }

    /// A heading too long for the sidebar must appear in full across wrapped
    /// rows, not be clipped at the edge. The last word of a long title is the
    /// proof: if wrapping worked, it is on screen somewhere.
    #[test]
    fn a_long_heading_wraps_instead_of_clipping() {
        let mut app = App::new();
        let doc = "# When intelligence itself becomes the product being shipped\n\nBody.\n";
        let parsed = markdown::render_markdown(doc, app.theme());
        app.blocks = parsed.blocks;
        app.headings = parsed.headings;

        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        // "shipped" only reaches the screen if the title wrapped; at 34
        // columns it would otherwise be clipped long before the last word.
        assert!(screen(&terminal).contains("shipped"), "the full heading should wrap into view");
    }
}
