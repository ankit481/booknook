# booknook

**A calm, book-like markdown reader for the terminal.**

Your notes deserve better than a scrollback buffer. booknook opens a markdown
file as a two-page spread, with a spine down the middle, generous margins,
and a palette chosen so the words come forward and everything else recedes.
It turns pages. It does not scroll.

```sh
cargo run
```

## Why

Every other terminal markdown viewer is a developer tool wearing a reader's
clothes. They scroll, they syntax-highlight, they fill the window edge to
edge, and they treat a long essay exactly the way they treat a log file.

booknook is built on the opposite premise. It is a reading device that
happens to live in a terminal, and every design decision follows from that.

**Pages, not scrolling.** An e-ink reader flips whole pages. It never leaves
you halfway between two of them. Neither does booknook. Every keypress moves
a full page, or a full spread, and lands cleanly.

**A book, not a wall of text.** Give it a wide terminal and it opens two
pages side by side on a single sheet, with the spine drawn on the paper
rather than as a gap between panels. Narrow the window and it becomes a
single page, the way a phone-sized e-reader would. The decision is remade
every frame, so resizing just works.

**Typography you can feel.** booknook word-wraps the text itself instead of
handing that job to the terminal, which is what lets it control the rhythm of
a page: the measure of the column, the air inside a paragraph, and the larger
gap between paragraphs. Those are three different numbers, and getting the
relationship between them right is most of what separates a page from a
transcript.

**Ink, not syntax highlighting.** Five palettes, cycled with a keypress, from
a cool Tokyo Night to a near-monochrome Ink to a genuinely light Paper. The
reading column sits on its own slightly lighter shade, so the text rests on a
sheet instead of bleeding into the terminal background.

**Keyboard only.** Real e-readers have buttons, not pointers. Mouse support
was left out on purpose.

## Features

- Full markdown rendering: headings, bold and italic, inline code, fenced
  code blocks with language labels, blockquotes, ordered and nested
  unordered lists, and links
- An always-visible file browser sidebar, so you can move between documents
  without leaving the reader
- Automatic two-page spread on wide terminals, single page on narrow ones
- Live typography controls: column width, line spacing, and paragraph
  spacing, all adjustable while reading
- Five color themes, cycled with a single key
- Correct handling of smart punctuation, so `country's` and `$78.02` render
  as words rather than as fragments with spaces wedged into them

## Install

Requires a recent stable Rust toolchain.

```sh
git clone https://github.com/ankit481/booknook
cd booknook
cargo build --release
```

The binary lands in `target/release/booknook`.

## Usage

With no arguments, booknook opens the file browser in the current directory.
Pass a path to open a file directly, or to start browsing somewhere specific.

```sh
booknook                    # browse from here
booknook path/to/notes      # browse a folder
booknook path/to/file.md    # open a document
```

### Keys

Available anywhere:

| Key | Action |
|---|---|
| `Tab` | Move focus between the sidebar and the reader |
| `t` | Cycle color theme |
| `q` / `Esc` | Quit |

In the sidebar:

| Key | Action |
|---|---|
| `↑` `↓` or `k` `j` | Move the selection |
| `→` / `l` / `Enter` | Open a folder or a markdown file |
| `←` / `h` / `Backspace` | Go up to the parent folder |

In the reader:

| Key | Action |
|---|---|
| `→` / `l` / space | Turn to the next page or spread |
| `←` / `h` / `Backspace` | Turn back |
| `g` / `G` | Jump to the first or last page |
| `-` / `+` | Narrow or widen the reading column |
| `[` / `]` | Less or more space between lines |
| `{` / `}` | Less or more space between paragraphs |
| `o` | Back to the sidebar |

## Getting the page right

booknook controls the column width, the paragraph rhythm, the margins, and
the color. It cannot control the font, and it cannot control the space
between two lines of glyphs. A terminal program writes characters into a
grid, and the size and shape of that grid belong to your terminal emulator.

This matters more than it sounds like it should. The two settings below are
what a typesetter would call leading and tracking, and no amount of work
inside the application can substitute for them.

**Windows Terminal.** Add a `font` block to your profile:

```json
"font": {
    "face": "Cascadia Mono",
    "size": 13,
    "cellHeight": "1.45",
    "cellWidth": "1.05"
},
"padding": "28, 16, 28, 12",
"antialiasingMode": "grayscale"
```

**Ghostty.** In `~/.config/ghostty/config`:

```
font-family = IBM Plex Mono
font-size = 14
adjust-cell-height = 45%
adjust-cell-width = 4%
window-padding-x = 24
window-padding-y = 14
window-padding-balance = true
```

Once your terminal is doing the leading, press `[` inside booknook until the
line spacing reads `0`, and `{` until the paragraph spacing reads `1`. Real
leading from the terminal plus paragraph gaps from booknook is as close to a
printed page as a terminal gets.

For the typeface itself, prefer a humanist monospace designed with reading in
mind over one designed for telling `l` from `1` at eight points. IBM Plex
Mono, iA Writer Duospace, and Recursive Mono Casual are all free and all read
like books. JetBrains Mono is a good, widely available second choice. Avoid
fonts with programming ligatures, which are a distraction in prose.

## Built with

pulldown-cmark for parsing, ratatui and crossterm for the terminal, and a
hand-rolled word-wrapper in between. No async, no unsafe, roughly a thousand
lines of Rust.

If you want to know how the pieces fit together, or you are learning Rust and
want a small real codebase to read, see [docs/architecture.md](docs/architecture.md).
It covers the parse-then-wrap-then-render pipeline, why pages are stored as
numbers rather than scroll offsets, and how ownership and borrowing show up
as concrete decisions throughout.

## License

MIT. See [LICENSE](LICENSE).
