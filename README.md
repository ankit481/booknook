# booknook

**A calm, book-like reader for the terminal. Markdown and EPUB.**

Your notes deserve better than a scrollback buffer, and your books deserve
better than a browser tab. booknook opens a markdown file or an EPUB as a
two-page spread, with a spine down the middle, generous margins, and a
palette chosen so the words come forward and everything else recedes. It
turns pages. It does not scroll.

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

**Ink, not syntax highlighting.** Fifteen palettes, cycled with a keypress:
the reading-room classics the editor world already settled on, Solarized,
Gruvbox, Catppuccin, Kanagawa, Rosé Pine, Flexoki, One Dark, Dracula, and
Tokyo Night, alongside a genuinely light Paper, the warm cream of Kindle's
Sepia, and a dim amber Nocturne for reading in the dark. The reading column
sits on its own slightly lighter shade, so the text rests on a sheet instead
of bleeding into the terminal background.

**Keyboard only.** Real e-readers have buttons, not pointers. Mouse support
was left out on purpose.

## Features

- Full markdown rendering: headings, bold and italic, inline code, fenced
  code blocks with language labels, blockquotes, ordered and nested
  unordered lists, and links
- EPUB reading: a whole book opens as one continuous document, in reading
  order, with the book's real title in place of its filename. Chapter
  headings feed the contents pane; when a book carries no usable headings,
  its own navigation file is used instead. Italics and bold survive even in
  Calibre conversions that encode them as styled spans
- A file browser sidebar that recedes while you read: the moment focus
  moves to the reader, the sidebar disappears and the page stands alone on
  the sheet, the way it would on an e-reader. Tab or `o` brings it back
- A table of contents below the browser, drawn from the open document's
  headings, that jumps straight to any of them and marks where you are
- Remembers your place. Every document reopens on the page you left it on,
  the way a Kindle returns to the book you were reading, and launching with
  no argument reopens the last file you had open
- Automatic two-page spread on wide terminals, single page on narrow ones
- Book-quality page breaks: a page never ends on a heading, never strands
  a paragraph's first line at its bottom or sends the last line alone onto
  the next page, and never cuts a table row through a wrapped cell. When
  the arithmetic break would land badly, the page ends a line or two early
  instead, the way a typesetter leaves a short page rather than a bad break
- Live typography controls: column width, line spacing, and paragraph
  spacing, all adjustable while reading
- Six color themes, cycled with a single key, including Kindle-style sepia
- Correct handling of smart punctuation, so `country's` and `$78.02` render
  as words rather than as fragments with spaces wedged into them
- Code blocks and ASCII diagrams keep their exact shape, clipped at the page
  edge rather than reflowed, so their alignment survives. When one is wider
  than the column, `,` and `.` pan it sideways, the keyboard version of a
  horizontal scrollbar, with `‹` and `›` marking what lies past each edge

## Install

Requires a recent stable Rust toolchain.

```sh
git clone https://github.com/ankit481/booknook
cd booknook
cargo build --release
```

The binary lands in `target/release/booknook`.

## Usage

With no arguments, booknook reopens the last document you had open, on the
page you left it on. The very first time, with nothing yet remembered, it
opens the file browser in the current directory instead. Pass a path to open
a file directly, or to start browsing somewhere specific.

```sh
booknook                    # reopen where you left off
booknook path/to/notes      # browse a folder
booknook path/to/file.md    # open a document
booknook path/to/book.epub  # open a book
```

### Keys

Available anywhere:

| Key | Action |
|---|---|
| `Tab` | Move focus: files, then contents, then the reader, then back |
| `t` | Cycle color theme |
| `r` | Reload the open document from disk |
| `q` / `Esc` | Quit |

In the file browser:

| Key | Action |
|---|---|
| `↑` `↓` or `k` `j` | Move the selection |
| `→` / `l` / `Enter` | Open a folder or a markdown file |
| `←` / `h` / `Backspace` | Go up to the parent folder |

In the contents list:

| Key | Action |
|---|---|
| `↑` `↓` or `k` `j` | Move the selection |
| `→` / `l` / `Enter` / space | Jump the reader to that heading |
| `g` / `G` | Jump to the first or last heading |
| `←` / `h` / `Backspace` | Back to the file browser |

In the reader:

| Key | Action |
|---|---|
| `→` / `l` / space | Turn to the next page or spread |
| `←` / `h` / `Backspace` | Turn back |
| `g` / `G` | Jump to the first or last page |
| `-` / `+` | Narrow or widen the reading column |
| `[` / `]` | Less or more space between lines |
| `{` / `}` | Less or more space between paragraphs |
| `,` / `.` | Pan wide code blocks sideways; a page turn resets |
| `o` | Back to the file browser |

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

booknook now starts with line spacing `0` and paragraph spacing `1`, which is
where you want it. The reason is worth stating, because it is the single
biggest thing separating comfortable reading from tiring reading. Legibility
research puts the ideal leading for body text at roughly 1.2 to 1.45 times the
type size. A terminal cell already carries whatever leading your emulator is
set to, so a single-spaced paragraph sits right in that band. Insert a blank
row between every line and the leading jumps past 2.0, and at that point the
eye can no longer make the return sweep to the start of the next line
reliably. It overshoots, has to re-fixate, and the rhythm that lets you read
without noticing you are reading breaks down. That is the loose, hard-to-focus
feeling of over-spaced text. So the terminal supplies the leading, and
booknook supplies only the larger gap between paragraphs, which is what your
eye actually uses to tell one block from the next.

If your terminal is not doing the leading and the lines feel cramped, press
`]` to add a blank row back. And if you prefer more air, it is one keystroke
away. The default is the recommendation, not a lock.

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
