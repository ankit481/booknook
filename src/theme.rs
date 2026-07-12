//! Color palettes.
//!
//! A theme is plain data. Nothing here has behavior, and nothing here
//! knows what a page or a sidebar is. The `ui` module decides what to
//! paint with each color, and the `markdown` module decides which role
//! each piece of text plays.
//!
//! `page` is deliberately a slightly different shade from `bg`. The
//! reading column is painted with `page` and everything around it with
//! `bg`, so the text sits on something that reads as a sheet of paper
//! rather than filling the whole terminal edge to edge.
//!
//! Most of these are not booknook's own inventions. Choosing colors that
//! stay comfortable for hours is a solved problem, and the solutions have
//! names: Solarized, Gruvbox, Catppuccin, and the rest of the palettes the
//! editor world has already converged on. Each entry below maps a
//! well-known palette's published values onto booknook's roles, using the
//! scheme's own darkest tone for the surround and its base for the page,
//! so the theme reads here the way it reads everywhere else.

use ratatui::style::Color;

pub(crate) struct Theme {
    pub(crate) name: &'static str,
    /// Everything behind and around the page: sidebar, gutter, status bar.
    pub(crate) bg: Color,
    /// The reading column itself.
    pub(crate) page: Color,
    /// Body text. Softened rather than pure white, because maximum
    /// contrast is tiring to read for any length of time.
    pub(crate) fg: Color,
    pub(crate) heading: Color,
    pub(crate) code: Color,
    /// Chrome and punctuation the eye should skip: bullets, the spine,
    /// the status bar, unselected file names.
    pub(crate) muted: Color,
    pub(crate) quote: Color,
    pub(crate) link: Color,
}

/// Cycled through with `t` while reading, dark themes first, then light. A
/// `static` rather than a `const` so that `&THEMES[i]` borrows for `'static`
/// and callers never have to thread a lifetime through.
pub(crate) static THEMES: [Theme; 6] = [
    // Cool and dark, in the Tokyo Night family. The blues are pulled well
    // back from where a syntax highlighter would put them.
    Theme {
        name: "Tokyo Night",
        bg: Color::Rgb(22, 22, 30),
        page: Color::Rgb(26, 27, 38),
        fg: Color::Rgb(200, 206, 224),
        heading: Color::Rgb(233, 236, 245),
        code: Color::Rgb(150, 170, 200),
        muted: Color::Rgb(88, 92, 116),
        quote: Color::Rgb(158, 164, 186),
        link: Color::Rgb(122, 182, 172),
    },
    // Catppuccin's dark flagship: soft pastels on a blue-charcoal base.
    // Mantle for the surround, Base for the page, Text for the ink, with
    // Lavender headings, Peach code, and Blue links, all straight from the
    // published palette.
    Theme {
        name: "Catppuccin Mocha",
        bg: Color::Rgb(24, 24, 37),
        page: Color::Rgb(30, 30, 46),
        fg: Color::Rgb(205, 214, 244),
        heading: Color::Rgb(180, 190, 254),
        code: Color::Rgb(250, 179, 135),
        muted: Color::Rgb(108, 112, 134),
        quote: Color::Rgb(166, 173, 200),
        link: Color::Rgb(137, 180, 250),
    },
    // Rosé Pine's main variant: dusk purples and soft rose on near-black.
    // Base and Surface carry the page, Iris the headings, Gold the code,
    // Foam the links.
    Theme {
        name: "Rosé Pine",
        bg: Color::Rgb(25, 23, 36),
        page: Color::Rgb(31, 29, 46),
        fg: Color::Rgb(224, 222, 244),
        heading: Color::Rgb(196, 167, 231),
        code: Color::Rgb(246, 193, 119),
        muted: Color::Rgb(110, 106, 134),
        quote: Color::Rgb(144, 140, 170),
        link: Color::Rgb(156, 207, 216),
    },
    // Light, for reading in daylight. This is the one that actually looks
    // like an e-ink screen, since e-ink is a reflective surface and is at
    // its best bright.
    Theme {
        name: "Paper",
        bg: Color::Rgb(222, 216, 202),
        page: Color::Rgb(240, 235, 222),
        fg: Color::Rgb(58, 54, 48),
        heading: Color::Rgb(28, 26, 22),
        code: Color::Rgb(92, 86, 74),
        muted: Color::Rgb(140, 132, 118),
        quote: Color::Rgb(92, 86, 76),
        link: Color::Rgb(86, 102, 120),
    },
    // Kindle's sepia: a warm cream page with soft brown ink, the classic
    // e-reader setting for long sessions and for reading at night without the
    // glare of a white background. The page is the exact tone Kindle uses,
    // and every other value is pulled onto the same warm brown axis so
    // nothing on the page reads as gray against it.
    Theme {
        name: "Sepia",
        bg: Color::Rgb(228, 216, 191),
        page: Color::Rgb(251, 240, 217),
        fg: Color::Rgb(91, 70, 54),
        heading: Color::Rgb(61, 47, 36),
        code: Color::Rgb(122, 96, 68),
        muted: Color::Rgb(176, 158, 128),
        quote: Color::Rgb(120, 96, 72),
        link: Color::Rgb(150, 98, 52),
    },
    // Flexoki's paper side: an off-white the tone of unbleached paper with
    // warm near-black ink, designed, like its dark twin, for prose first.
    Theme {
        name: "Flexoki Light",
        bg: Color::Rgb(242, 240, 229),
        page: Color::Rgb(255, 252, 240),
        fg: Color::Rgb(52, 51, 49),
        heading: Color::Rgb(16, 15, 15),
        code: Color::Rgb(188, 82, 21),
        muted: Color::Rgb(135, 133, 128),
        quote: Color::Rgb(87, 86, 83),
        link: Color::Rgb(36, 131, 123),
    },
];
