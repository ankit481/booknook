//! The page-turn animation: a corner curl.
//!
//! Turning a page is the one place booknook moves on its own. Everywhere
//! else a frame is drawn once and the program goes back to sleep until the
//! next key. The curl is the exception: it renders a short run of frames on a
//! timer, lifting the page from its bottom corner and sweeping a diagonal fold
//! across it, the next page showing through underneath as the old one peels
//! away, the way a thumb turns a leaf in a real book.
//!
//! A terminal cannot draw a smooth three-dimensional curl: there is no shading
//! within a cell and no way to mirror text onto the back of a lifting sheet. So
//! the curl is stylized rather than photoreal. The fold is a diagonal seam; the
//! lifting edge is a thin strip of blank paper catching light, and just past it
//! a band of shadow falls on the revealed page. Those two cues, a lit edge and
//! the shadow behind it, are what read as depth at this resolution, and they
//! keep the text legible instead of dissolving it into a blocky fold.
//!
//! Only the reading column moves. The sidebar and the status bar are painted
//! from the settled destination and held still, so the motion reads as a page
//! turning inside the book rather than the whole screen animating.
//!
//! The effect is deliberately cheap. Each frame recolors already rendered cells
//! between two buffers, no reflow and no re-parse, so the cost is a sweep over
//! the column and the frame pauses themselves. It is off by default and never
//! gets in the way: a burst of held keys is folded into a single turn by the
//! caller, and a turn that lands on the same page, such as pressing forward on
//! the last page, is detected and skipped.

use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::ui;

/// Which way the pages travel. A forward turn lifts the bottom-right corner
/// and sweeps the fold up toward the top-left; a back turn mirrors it, lifting
/// the bottom-left corner instead, so paging backward reads as the reverse
/// motion of paging forward.
#[derive(Clone, Copy)]
pub(crate) enum Direction {
    Forward,
    Back,
}

/// How many frames a turn is spread across, and the pause between them.
/// Nine frames at fifteen milliseconds is about a seventh of a second: long
/// enough to read as a turn, short enough to never feel in the way.
const FRAMES: u16 = 9;
const FRAME_PAUSE: Duration = Duration::from_millis(15);

/// Turn from `before`, the frame that was on screen when the turn key was
/// pressed, to whatever page `app` now points at. `area` is the full frame
/// rect that `before` was captured at.
///
/// The destination is rendered and captured inside the first `draw` call and
/// composited over in the same call, so the finished page is never flashed on
/// screen before the curl begins. If the reading column is unchanged, the turn
/// produced no visible move and the settled page is simply left in place.
pub(crate) fn turn(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    before: Buffer,
    area: Rect,
    dir: Direction,
) -> Result<()> {
    let doc = ui::document_area(area);
    if doc.width == 0 || doc.height == 0 {
        terminal.draw(|frame| ui::draw(frame, app))?;
        return Ok(());
    }

    let mut after_img = None;
    let mut moved = false;
    terminal.draw(|frame| {
        ui::draw(frame, app);
        let after = frame.buffer_mut().clone();
        // A turn that lands on the same content, such as forward on the last
        // page, leaves the column untouched and needs no animation. The frame
        // already holds the settled page, so leaving it as-is is correct.
        if column_changed(&before, &after, doc) {
            moved = true;
            curl(frame.buffer_mut(), &before, &after, area, doc, dir, 1);
        }
        after_img = Some(after);
    })?;

    if !moved {
        return Ok(());
    }
    let after = after_img.expect("a drawn frame always fills the buffer");

    sleep(FRAME_PAUSE);
    for step in 2..FRAMES {
        terminal.draw(|frame| {
            curl(frame.buffer_mut(), &before, &after, area, doc, dir, step);
        })?;
        sleep(FRAME_PAUSE);
    }

    // Settle on the destination, drawn fresh so the final frame is the real
    // page and not the last curl frame, which stops a hair short of complete.
    terminal.draw(|frame| ui::draw(frame, app))?;
    Ok(())
}

/// Whether the reading column differs between the two frames. Only the
/// document rect is compared: the status bar's page number always changes on a
/// turn, but that is not what the eye is watching, so it must not by itself
/// count as movement worth animating.
fn column_changed(before: &Buffer, after: &Buffer, doc: Rect) -> bool {
    for y in doc.y..doc.y + doc.height {
        for x in doc.x..doc.x + doc.width {
            if before.cell((x, y)) != after.cell((x, y)) {
                return true;
            }
        }
    }
    false
}

/// How thick the lifted paper edge and the shadow behind it are, measured
/// along the fold's diagonal where the whole column spans two units. The edge
/// is kept thin, a lit crease; the shadow is a little wider, since a soft band
/// of shade reads as depth where a hard line would just look like a seam.
const EDGE: f32 = 0.05;
const SHADOW: f32 = 0.16;

/// Paint one frame of the curl into `dst`. The whole frame is filled from the
/// settled destination first, which puts the sidebar and status bar in their
/// final state and holds them there, and then the reading column is redrawn
/// with the fold at `step`'s share of the way across.
///
/// The fold is a diagonal line sweeping from the lifted corner toward the far
/// one. A cell is placed by where it sits relative to that line: still flat and
/// showing the outgoing page, on the lifted edge, in the shadow the edge casts,
/// or out past it all where the incoming page has been revealed.
fn curl(dst: &mut Buffer, before: &Buffer, after: &Buffer, area: Rect, doc: Rect, dir: Direction, step: u16) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let (Some(src), Some(cell)) = (after.cell((x, y)), dst.cell_mut((x, y))) {
                *cell = src.clone();
            }
        }
    }

    // The fold runs along the anti-diagonal a = u + v, where u and v are the
    // cell's position across and down the column as fractions from zero to one,
    // so a runs from zero at the top-left corner to two at the bottom-right. As
    // the turn progresses the fold sweeps from the lifted corner (a = 2) to the
    // opposite one (a = 0).
    let uden = doc.width.saturating_sub(1).max(1) as f32;
    let vden = doc.height.saturating_sub(1).max(1) as f32;
    let fold = 2.0 * (1.0 - step as f32 / FRAMES as f32);

    for y in doc.y..doc.y + doc.height {
        for x in doc.x..doc.x + doc.width {
            let u = (x - doc.x) as f32 / uden;
            let v = (y - doc.y) as f32 / vden;
            // A back turn lifts the other corner, so its fold is the mirror of
            // the forward one across the column's vertical center line.
            let u = match dir {
                Direction::Forward => u,
                Direction::Back => 1.0 - u,
            };
            let delta = u + v - fold;

            if delta < -EDGE {
                // Still flat: the outgoing page, not yet reached by the fold.
                if let (Some(s), Some(cell)) = (before.cell((x, y)), dst.cell_mut((x, y))) {
                    *cell = s.clone();
                }
            } else if delta < 0.0 {
                // The lifting edge: a thin strip of the sheet's blank back,
                // brightened as if catching the light as it stands up.
                if let Some(cell) = dst.cell_mut((x, y)) {
                    let lit = lift(cell.bg);
                    cell.set_symbol(" ");
                    cell.set_bg(lit);
                    cell.set_fg(lit);
                }
            } else if delta < SHADOW {
                // The shadow the lifted edge casts on the page revealed beneath
                // it. The incoming text stays, dimmed, so the shadow falls over
                // real page rather than a blank band.
                if let Some(cell) = dst.cell_mut((x, y)) {
                    let fg = shade(cell.fg);
                    let bg = shade(cell.bg);
                    cell.set_fg(fg);
                    cell.set_bg(bg);
                }
            }
            // Past the shadow, the frame already holds the incoming page from
            // the fill above, so there is nothing more to do.
        }
    }
}

/// Darken a color to about half, for the shadow the lifted edge casts. Only RGB
/// colors are scaled; a named or indexed color is left alone, since there is no
/// meaningful way to dim it without knowing the terminal's palette.
fn shade(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(scale(r, 45), scale(g, 45), scale(b, 45)),
        other => other,
    }
}

/// Brighten a color toward white, for the lit edge of the lifting sheet.
fn lift(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(toward_white(r, 45), toward_white(g, 45), toward_white(b, 45)),
        other => other,
    }
}

fn scale(v: u8, pct: u16) -> u8 {
    (v as u16 * pct / 100) as u8
}

fn toward_white(v: u8, pct: u16) -> u8 {
    (v as u16 + (255 - v as u16) * pct / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paint every cell of `buf`'s `rect` with `sym`.
    fn fill(buf: &mut Buffer, rect: Rect, sym: &str) {
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                buf.cell_mut((x, y)).unwrap().set_symbol(sym);
            }
        }
    }

    /// The symbol at a single cell of `buf`.
    fn at(buf: &Buffer, x: u16, y: u16) -> &str {
        buf.cell((x, y)).unwrap().symbol()
    }

    /// Build a before/after pair over a square column: the old page is all `A`,
    /// the new page all `B`, the surround (everything outside `doc`) `S` in the
    /// settled frame so its hold-still is visible.
    fn pages(area: Rect, doc: Rect) -> (Buffer, Buffer) {
        let mut before = Buffer::empty(area);
        fill(&mut before, area, "X");
        fill(&mut before, doc, "A");
        let mut after = Buffer::empty(area);
        fill(&mut after, area, "S");
        fill(&mut after, doc, "B");
        (before, after)
    }

    /// A forward turn lifts the bottom-right corner. Partway through, the fold's
    /// far corner (top-left) still shows the outgoing page, the lifted corner
    /// (bottom-right) shows the page revealed beneath, and the surround is held
    /// at the settled frame.
    #[test]
    fn a_forward_curl_reveals_the_new_page_from_the_lifted_corner() {
        let area = Rect::new(0, 0, 12, 12);
        let doc = Rect::new(2, 0, 8, 8);
        let (before, after) = pages(area, doc);

        let mut dst = Buffer::empty(area);
        curl(&mut dst, &before, &after, area, doc, Direction::Forward, 4);

        assert_eq!(at(&dst, doc.x, doc.y), "A", "the un-lifted far corner still shows the old page");
        assert_eq!(at(&dst, doc.x + doc.width - 1, doc.y + doc.height - 1), "B", "the lifted corner reveals the new page");
        assert_eq!(at(&dst, area.x, area.y), "S", "the surround is held at the settled frame");
    }

    /// A back turn lifts the opposite corner, so the reveal grows from the
    /// bottom-left instead of the bottom-right.
    #[test]
    fn a_back_curl_lifts_the_mirror_corner() {
        let area = Rect::new(0, 0, 12, 12);
        let doc = Rect::new(2, 0, 8, 8);
        let (before, after) = pages(area, doc);

        let mut dst = Buffer::empty(area);
        curl(&mut dst, &before, &after, area, doc, Direction::Back, 4);

        assert_eq!(at(&dst, doc.x, doc.y + doc.height - 1), "B", "the lifted bottom-left corner reveals the new page");
        assert_eq!(at(&dst, doc.x + doc.width - 1, doc.y), "A", "the far top-right corner still shows the old page");
    }

    /// When the two pages are identical, as at the end of a book where a
    /// forward turn cannot move, there is nothing to animate.
    #[test]
    fn an_unchanged_column_reports_no_movement() {
        let area = Rect::new(0, 0, 13, 1);
        let doc = Rect::new(2, 0, 9, 1);
        let mut a = Buffer::empty(area);
        fill(&mut a, doc, "A");
        let mut b = Buffer::empty(area);
        fill(&mut b, doc, "A");
        assert!(!column_changed(&a, &b, doc));
        b.cell_mut((5, 0)).unwrap().set_symbol("Z");
        assert!(column_changed(&a, &b, doc));
    }
}
