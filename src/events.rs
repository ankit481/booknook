//! Turning keyboard input into state changes.
//!
//! Each handler here takes the current `App` and a `KeyCode` and decides
//! what should change. Nothing in this module touches the terminal or
//! draws anything. That is the `ui` module's job.

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::anim::Direction;
use crate::app::{App, Focus, MAX_PAGE_WIDTH, MAX_SPACING, MIN_PAGE_WIDTH};
use crate::browser::is_readable;

pub(crate) fn handle_events(app: &mut App) -> Result<()> {
    // A key coalescing left behind, if any, is handled before blocking for a
    // new one. The animation drains the input queue to fold a burst of held
    // turn keys into a single slide, and parks the first non-turn key it finds
    // here so it is acted on rather than lost.
    let key = if let Some(key) = app.pending_keys.pop_front() {
        key
    } else {
        // read() blocks until the next terminal event arrives, whether that is
        // a key press, a resize, or a mouse event.
        let Event::Key(key) = event::read()? else {
            return Ok(());
        };
        key
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
        KeyCode::Char('a') => {
            app.animate = !app.animate;
            return Ok(());
        }
        KeyCode::Char('r') => {
            app.reload();
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
                } else if is_readable(&entry.path) {
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

/// The direction a key turns the page, if it turns it at all. Shared between
/// the reader's key handler and the animation's coalescing, so both agree on
/// exactly which keys count as a page turn. Everything else, jumps and
/// typography included, returns `None` and is left un-animated.
pub(crate) fn turn_direction(code: KeyCode) -> Option<Direction> {
    match code {
        KeyCode::Char(' ' | 'l' | 'j') | KeyCode::Right | KeyCode::Down | KeyCode::PageDown => Some(Direction::Forward),
        KeyCode::Char('h' | 'k') | KeyCode::Left | KeyCode::Up | KeyCode::PageUp | KeyCode::Backspace => {
            Some(Direction::Back)
        }
        _ => None,
    }
}

/// How many columns one press of `,` or `.` pans wide code sideways. Big
/// enough to make progress across a wide diagram, small enough that the
/// eye keeps its place between presses.
const PAN_STEP: u16 = 8;

/// Turn `app.page` by one step in `dir`, clamped at zero. The upper bound is
/// not known here, since the last page depends on the viewport, so a forward
/// turn is left to be clamped at draw time the same way `G` is. Turning a
/// page also ends any sideways pan: the lean-in to inspect a wide figure is
/// over once the reader moves on.
fn turn_page(app: &mut App, dir: Direction) {
    let step = if app.spread { 2 } else { 1 };
    match dir {
        Direction::Forward => app.page = app.page.saturating_add(step),
        Direction::Back => app.page = app.page.saturating_sub(step),
    }
    app.pan = 0;
}

/// After a turn, fold any keys already waiting in the queue into the same
/// move, so a held arrow or a fast run of presses becomes one slide over the
/// whole distance rather than a backlog of single-page animations. Further
/// turn keys advance the page; the first key that is not a turn is parked for
/// the main loop to handle next, and everything else, releases and resizes
/// among it, is discarded as the noise of a fast turn.
pub(crate) fn coalesce_turns(app: &mut App) -> Result<()> {
    while event::poll(Duration::ZERO)? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match turn_direction(key.code) {
                Some(dir) => turn_page(app, dir),
                None => {
                    app.pending_keys.push_back(key);
                    break;
                }
            },
            _ => {}
        }
    }
    Ok(())
}

fn handle_document_key(app: &mut App, code: KeyCode) {
    // In a two-page spread, a page turn flips the whole spread, both
    // pages, the way it would with a real book, rather than just the one
    // page currently in view.
    if let Some(dir) = turn_direction(code) {
        turn_page(app, dir);
        // Flag the turn so the main loop knows to animate it, if animation is
        // on. Jumps and typography deliberately do not set this.
        app.page_turn = true;
        return;
    }
    match code {
        KeyCode::Char('g') => {
            app.page = 0;
            app.pan = 0;
        }
        // The last page number is not known until the `ui` module computes
        // it from the viewport, so this asks for "as far as possible" and
        // lets the draw step clamp it to something real.
        KeyCode::Char('G') => {
            app.page = u16::MAX;
            app.pan = 0;
        }
        KeyCode::Char('o') => app.focus = Focus::Files,

        // Pan wide code blocks and diagrams sideways, the keyboard version
        // of a horizontal scrollbar. Like `G`, the upper bound is not known
        // here: only layout knows the widest verbatim line at the current
        // width, so this asks for more and lets the draw step clamp it.
        KeyCode::Char('.' | '>') => app.pan = app.pan.saturating_add(PAN_STEP),
        KeyCode::Char(',' | '<') => app.pan = app.pan.saturating_sub(PAN_STEP),

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Panning steps sideways with `.` and back with `,`, and a page turn
    /// drops the pan entirely: the lean-in to a wide figure ends when the
    /// reader moves on.
    #[test]
    fn pan_steps_and_a_page_turn_resets_it() {
        let mut app = App::new();
        handle_document_key(&mut app, KeyCode::Char('.'));
        handle_document_key(&mut app, KeyCode::Char('.'));
        assert_eq!(app.pan, 2 * PAN_STEP);
        handle_document_key(&mut app, KeyCode::Char(','));
        assert_eq!(app.pan, PAN_STEP);

        handle_document_key(&mut app, KeyCode::Char(' '));
        assert_eq!(app.pan, 0, "turning the page ends the pan");
        assert!(app.page_turn, "the turn itself must still register");
    }
}
