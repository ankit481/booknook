//! booknook: a calm, book-like markdown reader for the terminal.
//!
//! See `docs/architecture.md` for how the pieces below fit together.

mod app;
mod browser;
mod events;
mod markdown;
mod theme;
mod ui;
mod wrap;

use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::DefaultTerminal;

use app::App;
use events::handle_events;
use ui::draw;

fn main() -> Result<()> {
    let mut app = App::new();

    // A CLI argument opens a file directly, with the sidebar starting in
    // its folder, or starts browsing in a directory. With neither, browse
    // from the current working directory.
    match std::env::args().nth(1).map(PathBuf::from) {
        Some(path) if path.is_dir() => app.enter_dir(path),
        Some(path) => {
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
            app.enter_dir(dir);
            app.load_file(&path)?;
        }
        None => app.enter_dir(std::env::current_dir()?),
    }

    // ratatui::init() enables raw mode and the alternate screen, and
    // installs a panic hook that restores the terminal if the app
    // crashes. restore() undoes it.
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
