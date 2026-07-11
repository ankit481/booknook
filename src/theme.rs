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
pub(crate) static THEMES: [Theme; 15] = [
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
    // Kanagawa, after Hokusai's wave: warm parchment ink on deep sumi
    // black. Fuji White for the body, Fuji Gray for the chrome, Old White
    // for quotes, Crystal Blue for links, Wave Aqua for code. The most
    // book-like of the modern dark schemes.
    Theme {
        name: "Kanagawa",
        bg: Color::Rgb(22, 22, 29),
        page: Color::Rgb(31, 31, 40),
        fg: Color::Rgb(220, 215, 186),
        heading: Color::Rgb(238, 232, 205),
        code: Color::Rgb(122, 168, 159),
        muted: Color::Rgb(114, 113, 105),
        quote: Color::Rgb(200, 192, 147),
        link: Color::Rgb(126, 156, 216),
    },
    // Flexoki's dark side, a palette designed for reading prose rather
    // than code: inky warm blacks under paper-toned text, with the
    // scheme's signature orange on code and its cyan on links.
    Theme {
        name: "Flexoki Dark",
        bg: Color::Rgb(16, 15, 15),
        page: Color::Rgb(28, 27, 26),
        fg: Color::Rgb(206, 205, 195),
        heading: Color::Rgb(242, 240, 229),
        code: Color::Rgb(218, 112, 44),
        muted: Color::Rgb(111, 110, 105),
        quote: Color::Rgb(183, 181, 172),
        link: Color::Rgb(58, 169, 159),
    },
    // Gruvbox dark, the retro warm standby: cream text on brown-gray,
    // almost no blue anywhere. Hard background for the surround, medium
    // for the page, faded aqua and blue for code and links.
    Theme {
        name: "Gruvbox Dark",
        bg: Color::Rgb(29, 32, 33),
        page: Color::Rgb(40, 40, 40),
        fg: Color::Rgb(235, 219, 178),
        heading: Color::Rgb(251, 241, 199),
        code: Color::Rgb(142, 192, 124),
        muted: Color::Rgb(146, 131, 116),
        quote: Color::Rgb(189, 174, 147),
        link: Color::Rgb(131, 165, 152),
    },
    // Solarized dark, the most deliberately engineered palette ever made:
    // its contrast steps were fixed in CIELAB before being converted to
    // RGB. Base03 surrounds Base02, the body sits at Base0, and the
    // scheme's blue and cyan take links and code.
    Theme {
        name: "Solarized Dark",
        bg: Color::Rgb(0, 43, 54),
        page: Color::Rgb(7, 54, 66),
        fg: Color::Rgb(131, 148, 150),
        heading: Color::Rgb(238, 232, 213),
        code: Color::Rgb(42, 161, 152),
        muted: Color::Rgb(88, 110, 117),
        quote: Color::Rgb(147, 161, 161),
        link: Color::Rgb(38, 139, 210),
    },
    // One Dark, Atom's default and by now everyone's: neutral gray-blue,
    // nothing loud, the theme equivalent of a quiet room.
    Theme {
        name: "One Dark",
        bg: Color::Rgb(33, 37, 43),
        page: Color::Rgb(40, 44, 52),
        fg: Color::Rgb(171, 178, 191),
        heading: Color::Rgb(215, 218, 224),
        code: Color::Rgb(86, 182, 194),
        muted: Color::Rgb(92, 99, 112),
        quote: Color::Rgb(157, 165, 180),
        link: Color::Rgb(97, 175, 239),
    },
    // Dracula, the loudest theme allowed in: purple headings, cyan links,
    // and its trademark green on code. More saturated than anything else
    // here, kept faithful to the spec rather than toned down, because a
    // muted Dracula would not be Dracula.
    Theme {
        name: "Dracula",
        bg: Color::Rgb(33, 34, 44),
        page: Color::Rgb(40, 42, 54),
        fg: Color::Rgb(248, 248, 242),
        heading: Color::Rgb(189, 147, 249),
        code: Color::Rgb(80, 250, 123),
        muted: Color::Rgb(98, 114, 164),
        quote: Color::Rgb(241, 250, 140),
        link: Color::Rgb(139, 233, 253),
    },
    // For reading in a dark room. Everything drops in brightness, not just
    // in hue: the page is barely lighter than black and the ink is a dim
    // amber, the tone of a phosphor terminal or a screen at full Night
    // Shift. Almost no blue light leaves the screen, so a late chapter
    // does not push sleep further away, and the low overall luminance
    // means no halation around the letters when the room is unlit.
    Theme {
        name: "Nocturne",
        bg: Color::Rgb(15, 13, 10),
        page: Color::Rgb(21, 18, 14),
        fg: Color::Rgb(190, 166, 128),
        heading: Color::Rgb(218, 198, 162),
        code: Color::Rgb(166, 146, 114),
        muted: Color::Rgb(104, 90, 68),
        quote: Color::Rgb(162, 142, 112),
        link: Color::Rgb(176, 134, 86),
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
    // Solarized light, the daylight twin, with the same engineered
    // contrast steps: Base2 around a Base3 page, Base00 for the body.
    Theme {
        name: "Solarized Light",
        bg: Color::Rgb(238, 232, 213),
        page: Color::Rgb(253, 246, 227),
        fg: Color::Rgb(101, 123, 131),
        heading: Color::Rgb(7, 54, 66),
        code: Color::Rgb(42, 161, 152),
        muted: Color::Rgb(147, 161, 161),
        quote: Color::Rgb(88, 110, 117),
        link: Color::Rgb(38, 139, 210),
    },
    // Gruvbox light: dark brown ink on warm parchment, the daylight side
    // of the same retro palette.
    Theme {
        name: "Gruvbox Light",
        bg: Color::Rgb(235, 219, 178),
        page: Color::Rgb(251, 241, 199),
        fg: Color::Rgb(60, 56, 54),
        heading: Color::Rgb(40, 40, 40),
        code: Color::Rgb(66, 123, 88),
        muted: Color::Rgb(146, 131, 116),
        quote: Color::Rgb(80, 73, 69),
        link: Color::Rgb(7, 102, 120),
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
