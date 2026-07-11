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

/// Cycled through with `t` while reading. A `static` rather than a `const`
/// so that `&THEMES[i]` borrows for `'static` and callers never have to
/// thread a lifetime through.
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
    // Almost no color at all. Every value sits on the gray axis, so
    // nothing on the page competes with the words for attention.
    Theme {
        name: "Ink",
        bg: Color::Rgb(18, 18, 20),
        page: Color::Rgb(24, 24, 27),
        fg: Color::Rgb(206, 206, 208),
        heading: Color::Rgb(240, 240, 242),
        code: Color::Rgb(170, 172, 176),
        muted: Color::Rgb(98, 98, 104),
        quote: Color::Rgb(168, 168, 172),
        link: Color::Rgb(150, 170, 180),
    },
    // Warm and low contrast, the closest of these to lamplight on paper.
    Theme {
        name: "Ember",
        bg: Color::Rgb(24, 22, 20),
        page: Color::Rgb(30, 27, 24),
        fg: Color::Rgb(208, 196, 178),
        heading: Color::Rgb(238, 228, 210),
        code: Color::Rgb(188, 176, 158),
        muted: Color::Rgb(112, 102, 90),
        quote: Color::Rgb(176, 164, 148),
        link: Color::Rgb(188, 152, 110),
    },
    Theme {
        name: "Nord",
        bg: Color::Rgb(36, 41, 51),
        page: Color::Rgb(46, 52, 64),
        fg: Color::Rgb(216, 222, 233),
        heading: Color::Rgb(236, 239, 244),
        code: Color::Rgb(143, 188, 187),
        muted: Color::Rgb(98, 110, 128),
        quote: Color::Rgb(180, 190, 206),
        link: Color::Rgb(136, 192, 208),
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
];
