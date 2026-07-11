//! booknook: a calm, book-like markdown reader for the terminal.
//!
//! See `docs/architecture.md` for how the pieces below fit together.

mod app;
mod browser;
mod events;
mod gist;
mod markdown;
mod pr;
mod session;
mod theme;
mod ui;
mod wrap;

use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::DefaultTerminal;

use app::App;
use events::handle_events;
use session::Session;
use ui::draw;

fn main() -> Result<()> {
    let mut app = App::new();

    // The saved session carries the typography, theme, and remembered pages
    // from last time. Applying it before opening anything means the first
    // document already loads at its remembered page and settings.
    let session = Session::load();
    app.apply_session(&session);
    open_initial(&mut app, &session)?;

    // ratatui::init() enables raw mode and the alternate screen, and
    // installs a panic hook that restores the terminal if the app
    // crashes. restore() undoes it.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();

    // Write the session back out before returning, so quitting from any
    // page saves it. The current page is folded in first, then the whole
    // state is handed to `session::save`. A failure to save is deliberately
    // ignored: it must not mask the program's real exit status.
    app.remember_position();
    let _ = app.to_session().save();

    result
}

/// Decide what to show on launch. An explicit argument always wins: a gist or
/// other markdown URL is fetched and read, a directory opens the browser
/// there, and a file opens the reader. With no argument, the document from
/// last time is reopened at its remembered page, the way a Kindle returns to
/// the book it was closed on. If nothing was open before, or that file has
/// since gone, browsing starts from the current directory.
fn open_initial(app: &mut App, session: &Session) -> Result<()> {
    // A URL is handled before the path branches, since it is neither a
    // directory nor a file to canonicalize. The sidebar still needs
    // somewhere to point, both panes are always drawn, so it lists the
    // current directory while the reader shows the fetched document.
    //
    // A pull-request link is checked before a plain remote link, because a PR
    // URL is also an https URL: it needs the authenticated `gh` path, not the
    // anonymous fetch a gist uses.
    if let Some(arg) = std::env::args().nth(1) {
        if pr::looks_like_pr(&arg) {
            let (title, raw) = pr::fetch(&arg)?;
            app.enter_dir(std::env::current_dir()?);
            app.load_content(raw, title);
            return Ok(());
        }
        if gist::looks_remote(&arg) {
            let raw = gist::fetch(&arg)?;
            app.enter_dir(std::env::current_dir()?);
            app.load_content(raw, gist::title(&arg));
            return Ok(());
        }
    }

    match std::env::args().nth(1).map(PathBuf::from) {
        Some(path) if path.is_dir() => app.enter_dir(path),
        Some(path) => {
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
            app.enter_dir(dir);
            app.load_file(&path)?;
        }
        None => match &session.last_file {
            Some(file) if file.is_file() => {
                let dir = file.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
                app.enter_dir(dir);
                // A file that no longer reads cleanly falls back to browsing
                // rather than failing to start.
                if app.load_file(file).is_err() {
                    app.enter_dir(std::env::current_dir()?);
                }
            }
            _ => app.enter_dir(std::env::current_dir()?),
        },
    }
    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.quit {
        terminal.draw(|frame| draw(frame, app))?;
        handle_events(app)?;
    }
    Ok(())
}
