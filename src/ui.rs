//! Rendering the application state to the terminal.
//!
//! Every function here takes the current `App` and a ratatui `Frame`, and
//! draws. A couple of functions take `&mut App`, because they need to
//! clamp a value like the current page once the true page count is known,
//! but nothing here changes application state beyond that kind of
//! clamping. Turning input into state changes is the `events` module's
//! job.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::browser::is_markdown;
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

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(1)])
        .split(body);
    let (sidebar_area, doc_area) = (cols[0], cols[1]);

    draw_sidebar(frame, app, sidebar_area);
    let page_count = draw_document(frame, app, doc_area);
    draw_status_bar(frame, app, status_bar, page_count);
}

/// The gap between the two pages of a spread, like a book's spine.
const GUTTER: u16 = 5;

fn draw_document(frame: &mut Frame, app: &mut App, area: Rect) -> u16 {
    let theme = app.theme();
    if app.blocks.is_empty() {
        app.spread = false;
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
    // between wrapped lines for real line spacing. Because this runs
    // every frame against the current width, resizing the terminal
    // reflows the text correctly for free.
    let text = wrap::layout(&app.blocks, reading_area.width, app.spacing);
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(theme.fg).bg(theme.page))
        .wrap(Wrap { trim: false });

    // line_count() reports how many rows the text needs at this width. Our
    // own wrapping already fits `reading_area.width`, so this is normally
    // just the number of lines already produced, but it also catches the
    // rare case of a single word too long to fit, such as a long URL,
    // which Paragraph's own wrap still has to hard-break as a safety net.
    let viewport = reading_area.height.max(1);
    let total_rows = paragraph.line_count(reading_area.width) as u16;
    let page_count = total_rows.div_ceil(viewport).max(1);
    app.page = app.page.min(page_count - 1);

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

    let text = wrap::layout(&app.blocks, left_area.width, app.spacing);
    let viewport = left_area.height.max(1);
    let total_rows = text.lines.len() as u16;
    let page_count = total_rows.div_ceil(viewport).max(1);

    app.page = app.page.min(page_count.saturating_sub(1));
    if app.page % 2 == 1 {
        app.page -= 1;
    }

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

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = app.theme();
    let focused = matches!(app.focus, Focus::Sidebar);
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
        let is_selected = i == app.selected;
        let name = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.clone() };
        let color = if entry.is_dir {
            theme.link
        } else if is_markdown(&entry.path) {
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

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, page_count: u16) {
    let theme = app.theme();
    let text = match app.focus {
        Focus::Sidebar => {
            format!("  ↑/↓ move · →/l open · ←/h up · Tab reader · t {} · q quit", theme.name)
        }
        Focus::Document => {
            let position = if app.spread && app.page + 1 < page_count {
                format!("pages {}-{}", app.page + 1, app.page + 2)
            } else {
                format!("page {}", app.page + 1)
            };
            format!(
                "  {position} / {page_count}   line {} · para {} · width {} · {}   [ ] {{ }} -/+ t · Tab · q",
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
