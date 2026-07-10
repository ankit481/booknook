//! Application state: what booknook currently knows and is showing.
//!
//! `App` is the single source of truth. Nothing outside this module writes
//! to its fields as a side effect of some unrelated operation. State
//! changes that need to keep several fields consistent with each other,
//! such as `dir` and `entries`, or `blocks` and `page`, go through a
//! method here, so that consistency lives in one place.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::browser::{self, Entry};
use crate::markdown::{self, RenderLine};

/// Which pane currently receives keyboard input. Both panes are always
/// drawn; this only decides where `j`, `k`, the arrow keys, and Enter go.
pub(crate) enum Focus {
    Sidebar,
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
    /// The parsed document, not yet wrapped to any particular width.
    /// Layout happens every frame, against the current column width, in
    /// the `ui` module. This stays empty until a file has been opened.
    pub(crate) blocks: Vec<RenderLine>,
    pub(crate) page: u16,
    /// Whether the last draw showed two pages side by side. Set by the
    /// `ui` module every frame, since it is the only place that knows the
    /// current width, and read by the event handler to decide whether a
    /// page turn should move by one page or by a whole spread.
    pub(crate) spread: bool,
    pub(crate) quit: bool,
}

impl App {
    pub(crate) fn new() -> Self {
        App {
            focus: Focus::Sidebar,
            dir: PathBuf::new(),
            entries: Vec::new(),
            selected: 0,
            title: String::new(),
            blocks: Vec::new(),
            page: 0,
            spread: false,
            quit: false,
        }
    }

    /// Read `path`, parse it, and switch keyboard focus to the reader.
    pub(crate) fn load_file(&mut self, path: &Path) -> Result<()> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        self.title = path.display().to_string();
        self.blocks = markdown::render_markdown(&raw);
        self.page = 0;
        self.focus = Focus::Document;
        Ok(())
    }

    /// Point the sidebar at `dir` and list its contents.
    pub(crate) fn enter_dir(&mut self, dir: PathBuf) {
        self.entries = browser::list_dir(&dir);
        self.dir = dir;
        self.selected = 0;
    }
}
