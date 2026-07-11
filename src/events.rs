//! Turning keyboard input into state changes.
//!
//! Each handler here takes the current `App` and a `KeyCode` and decides
//! what should change. Nothing in this module touches the terminal or
//! draws anything. That is the `ui` module's job.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::app::{App, Focus, MAX_PAGE_WIDTH, MAX_SPACING, MIN_PAGE_WIDTH};
use crate::browser::is_markdown;

pub(crate) fn handle_events(app: &mut App) -> Result<()> {
    // read() blocks until the next terminal event arrives, whether that is
    // a key press, a resize, or a mouse event.
    let Event::Key(key) = event::read()? else {
        return Ok(());
    };
    // On Windows, keys fire both a press and a release event. Releases are
    // ignored.
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    // Quitting and switching focus work from either pane, so they are
    // handled once here instead of being duplicated in both key handlers.
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.quit = true;
            return Ok(());
        }
        KeyCode::Tab => {
            // Files, then the contents list, then the reader, then back. The
            // contents step is skipped when the open document has no headings,
            // or none is open, so Tab never lands on an empty pane.
            app.focus = match app.focus {
                Focus::Files if !app.headings.is_empty() => Focus::Toc,
                Focus::Files => Focus::Document,
                Focus::Toc => Focus::Document,
                Focus::Document => Focus::Files,
            };
            return Ok(());
        }
        KeyCode::Char('t') => {
            app.cycle_theme();
            return Ok(());
        }
        _ => {}
    }

    match app.focus {
        Focus::Files => handle_files_key(app, key.code),
        Focus::Toc => {
            handle_toc_key(app, key.code);
            Ok(())
        }
        Focus::Document => {
            handle_document_key(app, key.code);
            Ok(())
        }
    }
}

/// Move the selection, descend into a directory, go back up, or open a
/// markdown file. Right, `l`, and Enter mean "go deeper." Left, `h`, and
/// Backspace mean "go back," which are the same directions the reader uses
/// for page turns.
fn handle_files_key(app: &mut App, code: KeyCode) -> Result<()> {
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
                app.enter_dir(parent);
            }
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            if let Some(entry) = app.entries.get(app.selected) {
                if entry.is_dir {
                    let target = entry.path.clone();
                    app.enter_dir(target);
                } else if is_markdown(&entry.path) {
                    let target = entry.path.clone();
                    app.load_file(&target)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Move through the contents list and jump to a heading. Right, `l`, Enter,
/// and space all mean "take me there," matching the reader's own
/// forward-motion keys. Left and `h` step back to the file list, the same
/// direction that goes up a folder.
fn handle_toc_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.headings.is_empty() {
                app.toc_selected = (app.toc_selected + 1).min(app.headings.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => app.toc_selected = app.toc_selected.saturating_sub(1),
        KeyCode::Char('g') => app.toc_selected = 0,
        KeyCode::Char('G') => app.toc_selected = app.headings.len().saturating_sub(1),
        KeyCode::Char('l' | ' ') | KeyCode::Right | KeyCode::Enter => app.jump_to_heading(app.toc_selected),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => app.focus = Focus::Files,
        _ => {}
    }
}

fn handle_document_key(app: &mut App, code: KeyCode) {
    // In a two-page spread, a page turn flips the whole spread, both
    // pages, the way it would with a real book, rather than just the one
    // page currently in view.
    let step = if app.spread { 2 } else { 1 };
    match code {
        KeyCode::Char(' ' | 'l' | 'j') | KeyCode::Right | KeyCode::Down | KeyCode::PageDown => {
            app.page = app.page.saturating_add(step);
        }
        KeyCode::Char('h' | 'k') | KeyCode::Left | KeyCode::Up | KeyCode::PageUp | KeyCode::Backspace => {
            app.page = app.page.saturating_sub(step);
        }
        KeyCode::Char('g') => app.page = 0,
        // The last page number is not known until the `ui` module computes
        // it from the viewport, so this asks for "as far as possible" and
        // lets the draw step clamp it to something real.
        KeyCode::Char('G') => app.page = u16::MAX,
        KeyCode::Char('o') => app.focus = Focus::Files,

        // Typography, adjustable while reading. Changing any of these
        // reflows the document on the next frame, which can move the text
        // currently on screen onto a different page, so they deliberately
        // leave `page` alone rather than trying to preserve a position.
        KeyCode::Char('[') => app.spacing.line = app.spacing.line.saturating_sub(1),
        KeyCode::Char(']') => app.spacing.line = (app.spacing.line + 1).min(MAX_SPACING),
        KeyCode::Char('{') => app.spacing.paragraph = app.spacing.paragraph.saturating_sub(1),
        KeyCode::Char('}') => app.spacing.paragraph = (app.spacing.paragraph + 1).min(MAX_SPACING),
        KeyCode::Char('-') => app.page_width = app.page_width.saturating_sub(2).max(MIN_PAGE_WIDTH),
        KeyCode::Char('=' | '+') => app.page_width = (app.page_width + 2).min(MAX_PAGE_WIDTH),
        _ => {}
    }
}
