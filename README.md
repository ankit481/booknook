# booknook

A calm, book-like markdown reader for the terminal.

## Philosophy

Most terminal markdown viewers try to be editors, or dashboards, or dense
developer tools. `booknook` tries to be none of those things. It tries to be
a *reading device* -- closer to an e-ink reader than to a code viewer.

That shows up as a few deliberate constraints:

- **Pages, not scrolling.** An e-ink reader doesn't scroll; it flips whole
  pages. `booknook` never scrolls a document mid-page either -- every
  keypress moves you a full page (or a full spread) at a time.
- **A book, not a wall of text.** Wide enough terminal, and `booknook` opens
  a two-page spread with a spine down the middle, the way an actual open
  book looks. Narrow it, and it falls back to a single page, the way a
  phone-sized e-reader would.
- **Real line spacing.** Text is word-wrapped by the app itself (not left to
  the terminal), specifically so a blank row can be inserted between every
  wrapped line -- the same effect as an e-reader's line-spacing setting.
- **A quiet palette.** A cool, bluish-black background in the spirit of
  Tokyo Night / One Dark, with crisp, high-contrast body text and almost no
  other color. The goal is ink on a page, not syntax highlighting.
- **Keyboard only.** No mouse handling, on purpose. Real e-readers are
  button-driven, not pointer-driven, and that fits a calm reading tool
  better than a click target does.

It also doubles as a small, from-scratch Rust project: a markdown parser
(via `pulldown-cmark`) feeding a hand-rolled word-wrapper, rendered with
`ratatui` and `crossterm`, with an in-app file browser instead of a native
file dialog.

## Features

- Full markdown rendering: headings, bold/italic, inline code, fenced code
  blocks (with language label), blockquotes, ordered and nested unordered
  lists, and links
- A NeoTree-style file browser sidebar, always visible alongside the reader
- Two-page book spread on wide terminals, single page on narrow ones,
  decided automatically every frame
- Real word-wrap and line spacing, computed by the app rather than left to
  the terminal
- Keyboard-only navigation, with `Tab` to move focus between the sidebar and
  the reader

## Usage

```sh
cargo run
```

With no arguments, it opens the file browser in the current directory. Pass
a path to open a specific file or start browsing a specific folder:

```sh
cargo run -- path/to/notes
cargo run -- path/to/file.md
```

### Keybindings

**Sidebar**

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Move selection |
| `→` / `l` / `Enter` | Open folder or markdown file |
| `←` / `h` / `Backspace` | Go to parent folder |
| `Tab` | Switch focus to the reader |
| `q` / `Esc` | Quit |

**Reader**

| Key | Action |
|---|---|
| `→` / `l` / `j` / space | Next page (or next spread) |
| `←` / `h` / `k` / `Backspace` | Previous page (or previous spread) |
| `g` / `G` | First / last page |
| `Tab` / `o` | Switch focus to the sidebar |
| `q` / `Esc` | Quit |

## Building

Requires a recent stable Rust toolchain.

```sh
cargo build --release
```

## License

MIT -- see [LICENSE](LICENSE).
