//! Application state: what booknook currently knows and is showing.
//!
//! `App` is the single source of truth. Nothing outside this module writes
//! to its fields as a side effect of some unrelated operation. State
//! changes that need to keep several fields consistent with each other,
//! such as `dir` and `entries`, or `blocks` and `page`, go through a
//! method here, so that consistency lives in one place.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::browser::{self, Entry};
use crate::markdown::{self, Heading, RenderLine};
use crate::session::Session;
use crate::theme::{Theme, THEMES};
use crate::wrap::Spacing;

/// Which pane currently receives keyboard input. All panes are always drawn;
/// this only decides where `j`, `k`, the arrow keys, and Enter go. `Files`
/// and `Toc` both live in the sidebar, one above the other, and focus moves
/// between them and the reader with Tab.
pub(crate) enum Focus {
    Files,
    Toc,
    Document,
}

/// Everything the running app needs to know.
///
/// The sidebar and the document are not mutually exclusive. Both are
/// always on screen, so their state lives side by side in one `App`
/// rather than behind an enum. `focus` is the only thing that still works
/// like a mode switch, because exactly one pane owns the keyboard at a
/// time.
///
/// The reader stores a page number, not a scroll row. An e-ink reader
/// flips whole pages and never scrolls mid-page. The row offset for a
/// given page is derived at draw time from the current viewport height, so
/// `page` stays meaningful even if the terminal is resized between
/// frames.
pub(crate) struct App {
    pub(crate) focus: Focus,
    pub(crate) dir: PathBuf,
    pub(crate) entries: Vec<Entry>,
    pub(crate) selected: usize,
    pub(crate) title: String,
    /// The path of the open document, if any. Kept apart from `title`, which
    /// is only for display: this is the canonical key under which the reading
    /// position is remembered, and the file to reopen on the next launch.
    pub(crate) current: Option<PathBuf>,
    /// The open document's raw markdown, kept so that switching themes can
    /// reparse it. Colors are baked into `blocks` at parse time, so a new
    /// theme means a new parse.
    raw: String,
    /// The parsed document, not yet wrapped to any particular width.
    /// Layout happens every frame, against the current column width, in
    /// the `ui` module. This stays empty until a file has been opened.
    pub(crate) blocks: Vec<RenderLine>,
    /// The open document's headings, shown in the sidebar as a table of
    /// contents. Each carries the block it points into, so selecting one can
    /// be resolved to a page at draw time.
    pub(crate) headings: Vec<Heading>,
    /// Which contents entry the sidebar's cursor is on, when the contents
    /// pane has focus.
    pub(crate) toc_selected: usize,
    /// The heading the current page falls under, if any. Written by the `ui`
    /// module every frame, since only it knows the row each heading lands on
    /// at the current width, and read back by the same module to highlight
    /// that entry in the contents list. This is the same kind of
    /// draw-time-to-draw-time channel as `spread`.
    pub(crate) active_heading: Option<usize>,
    /// A pending request to jump the reader to a particular block, set when a
    /// contents entry is chosen. The `ui` module consumes it on the next
    /// frame, once it knows the viewport height needed to turn the block's
    /// row into a page, and clears it back to `None`. This mirrors how `G`
    /// asks for the last page without computing it here.
    pub(crate) pending_jump: Option<usize>,
    pub(crate) page: u16,
    /// Whether the last draw showed two pages side by side. Set by the
    /// `ui` module every frame, since it is the only place that knows the
    /// current width, and read by the event handler to decide whether a
    /// page turn should move by one page or by a whole spread.
    pub(crate) spread: bool,
    /// How the page is set: the width of a reading column, and how much
    /// air goes between lines and between paragraphs. These are adjustable
    /// while reading, because the right values are a matter of taste and
    /// of the font the terminal happens to be using.
    pub(crate) page_width: u16,
    pub(crate) spacing: Spacing,
    /// The page last reached in every document opened, keyed by canonical
    /// path. Loaded from the saved session at startup and written back on
    /// quit, so returning to any file lands on the page it was left on. Only
    /// `current`'s entry is updated live; the rest carry over untouched.
    positions: HashMap<PathBuf, u16>,
    /// An index into `THEMES` rather than a `Theme` value, so that `App`
    /// borrows the palette instead of owning a copy of it.
    theme_index: usize,
    pub(crate) quit: bool,
}

/// The range each typographic setting is allowed to move within.
pub(crate) const MIN_PAGE_WIDTH: u16 = 40;
pub(crate) const MAX_PAGE_WIDTH: u16 = 96;
pub(crate) const MAX_SPACING: u16 = 3;

impl App {
    pub(crate) fn new() -> Self {
        App {
            focus: Focus::Files,
            dir: PathBuf::new(),
            entries: Vec::new(),
            selected: 0,
            title: String::new(),
            current: None,
            raw: String::new(),
            blocks: Vec::new(),
            headings: Vec::new(),
            toc_selected: 0,
            active_heading: None,
            pending_jump: None,
            page: 0,
            spread: false,
            page_width: 58,
            // No blank row inside a paragraph, one between paragraphs. A
            // terminal cell already carries whatever leading the emulator is
            // configured for, so a single-spaced paragraph reads like book
            // body text; adding a blank row between every line pushes the
            // leading past where the eye can still track the return sweep to
            // the next line, which is what makes long reading tiring. The
            // paragraph gap alone is enough to keep the structure legible.
            spacing: Spacing { line: 0, paragraph: 1 },
            positions: HashMap::new(),
            theme_index: 0,
            quit: false,
        }
    }

    /// The palette currently in use. The returned reference borrows from
    /// the `THEMES` static, not from `self`, so holding onto it does not
    /// keep `App` borrowed.
    pub(crate) fn theme(&self) -> &'static Theme {
        &THEMES[self.theme_index]
    }

    /// Move to the next palette and reparse the open document with it.
    pub(crate) fn cycle_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % THEMES.len();
        if !self.raw.is_empty() {
            let parsed = markdown::render_markdown(&self.raw, self.theme());
            self.blocks = parsed.blocks;
            self.headings = parsed.headings;
        }
    }

    /// Read `path`, parse it, and switch keyboard focus to the reader. The
    /// page it opens on is whatever was last reached in this file, so
    /// reopening a document resumes it rather than restarting it.
    pub(crate) fn load_file(&mut self, path: &Path) -> Result<()> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        // Record where the previously open file was left before moving on, so
        // a session that touches several files remembers each of them.
        self.remember_position();

        let parsed = markdown::render_markdown(&raw, self.theme());
        self.title = path.display().to_string();
        self.current = Some(canonical(path));
        self.blocks = parsed.blocks;
        self.headings = parsed.headings;
        self.raw = raw;
        self.toc_selected = 0;
        self.active_heading = None;
        self.pending_jump = None;
        self.page = self.saved_page(path);
        self.focus = Focus::Document;
        Ok(())
    }

    /// Open already-fetched text in the reader, the way `load_file` opens a
    /// file, but without a path behind it. This is how a gist is shown: the
    /// bytes have already come off the network, so there is nothing to read
    /// from disk. `title` is whatever should appear in the reader's header in
    /// place of a filename.
    ///
    /// `current` is deliberately left `None`. A gist has no canonical path, so
    /// it is neither remembered as the file to reopen next launch nor given an
    /// entry in the per-file position map; both of those are keyed by path.
    /// The trade-off is that a gist always opens on its first page, which is
    /// the right default for something reached by pasting a link rather than
    /// returned to like a book on a shelf.
    pub(crate) fn load_content(&mut self, raw: String, title: String) {
        // Fold away the previously open file's page before replacing it, the
        // same courtesy `load_file` extends, so opening a gist mid-session
        // does not lose the place in whatever was open before.
        self.remember_position();

        let parsed = markdown::render_markdown(&raw, self.theme());
        self.title = title;
        self.current = None;
        self.blocks = parsed.blocks;
        self.headings = parsed.headings;
        self.raw = raw;
        self.toc_selected = 0;
        self.active_heading = None;
        self.pending_jump = None;
        self.page = 0;
        self.focus = Focus::Document;
    }

    /// Point the sidebar at `dir` and list its contents.
    pub(crate) fn enter_dir(&mut self, dir: PathBuf) {
        self.entries = browser::list_dir(&dir);
        self.dir = dir;
        self.selected = 0;
    }

    /// Ask the reader to move to the heading at `toc_index` in the contents
    /// list. The actual page is worked out at draw time, so this only records
    /// the target block and hands focus to the reader.
    pub(crate) fn jump_to_heading(&mut self, toc_index: usize) {
        if let Some(heading) = self.headings.get(toc_index) {
            self.pending_jump = Some(heading.block);
            self.focus = Focus::Document;
        }
    }

    /// Store the current page under the open file, so it can be resumed
    /// later. A no-op when nothing is open.
    pub(crate) fn remember_position(&mut self) {
        if let Some(current) = &self.current {
            self.positions.insert(current.clone(), self.page);
        }
    }

    /// Restore adjustable settings and remembered positions from a saved
    /// session. Called once at startup, before any file is opened, so the
    /// restored width, spacing, and theme are already in force when the first
    /// document is loaded.
    pub(crate) fn apply_session(&mut self, session: &Session) {
        self.page_width = session.page_width.clamp(MIN_PAGE_WIDTH, MAX_PAGE_WIDTH);
        self.spacing = Spacing {
            line: session.line.min(MAX_SPACING),
            paragraph: session.para.min(MAX_SPACING),
        };
        self.theme_index = session.theme_index % THEMES.len();
        self.positions = session.positions.clone();
    }

    /// Capture the current settings and positions as a `Session` to be
    /// written out on quit. The caller is expected to have called
    /// `remember_position` first, so the open file's latest page is included.
    pub(crate) fn to_session(&self) -> Session {
        Session {
            last_file: self.current.clone(),
            page_width: self.page_width,
            line: self.spacing.line,
            para: self.spacing.paragraph,
            theme_index: self.theme_index,
            positions: self.positions.clone(),
        }
    }

    /// The page remembered for `path`, or zero if it has never been opened.
    fn saved_page(&self, path: &Path) -> u16 {
        self.positions.get(&canonical(path)).copied().unwrap_or(0)
    }
}

/// The canonical form of a path, used as the stable key for a document's
/// remembered position. Canonicalizing means the same file reached by a
/// relative path, an absolute one, or through a symlink all resolve to one
/// entry rather than three. If the path cannot be canonicalized, for
/// instance because it no longer exists, its own form is used as-is, which is
/// still a consistent key for the life of the process.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
