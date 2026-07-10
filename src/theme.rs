//! The color palette.
//!
//! A cool, bluish-black background in the spirit of Tokyo Night and One
//! Dark, paired with crisp, near-white body text. The goal is an
//! IDE-dark-theme backdrop with Kindle-crisp reading text.

use ratatui::style::Color;

pub(crate) const BG: Color = Color::Rgb(22, 23, 34);
pub(crate) const FG: Color = Color::Rgb(214, 217, 226);
pub(crate) const HEADING: Color = Color::Rgb(242, 243, 247);
pub(crate) const CODE: Color = Color::Rgb(140, 170, 220);
pub(crate) const MUTED: Color = Color::Rgb(94, 98, 122);
pub(crate) const QUOTE: Color = Color::Rgb(158, 163, 184);
pub(crate) const LINK: Color = Color::Rgb(122, 196, 178);
