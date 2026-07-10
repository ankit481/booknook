# Vellum

A calm markdown reader for the terminal.

This is a **sample document** so you can see how *emphasis*, `inline code`,
and headings render in the reading column.

## Why a reading column

Long lines are tiring to read. Newspapers and books keep text in a narrow
column for a reason: your eye finds the next line more easily. Vellum caps the
text width and centers it, so even on a wide monitor the page stays restful.

## Things to try

- Press `→` or space to flip to the next page
- Press `←` to flip back
- Press `g` / `G` to jump to the first or last page
- Press `o` to open a different file
- Press `q` to quit

## Lists, ordered and nested

1. First steps
2. Second steps
   - a nested detail
   - another nested detail
3. Third steps

## A quiet aside

> Margins, measure, and a soft contrast between ink and paper all work
> together so the words can recede and the meaning can come forward.

## A little code

Here's how the page column gets centered:

```rust
fn centered_column(area: Rect, max_width: u16) -> Rect {
    let width = area.width.min(max_width);
    let margin = (area.width - width) / 2;
    Rect { x: area.x + margin, y: area.y, width, height: area.height }
}
```

## One more thing

You can read more about the parser this project is built on at
[pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark).
