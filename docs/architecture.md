# Architecture

This document explains how booknook is put together. It assumes you have
read the README and understand what the app is trying to be. Here we focus
on how the code delivers on that, module by module.

## The pipeline, from markdown text to a terminal frame

Opening a file sets three things in motion, and it helps to understand them
as a pipeline before looking at any single module.

First, the file's text is parsed. The `markdown` module reads the raw
markdown string with pulldown-cmark and walks its stream of events, turning
headings, paragraphs, lists, quotes, links, and code blocks into a flat list
of `RenderLine` values. Nothing in this step knows how wide the terminal
is. A `RenderLine` is one of three things: `Prose`, a run of styled spans
that can still be reflowed later; `Verbatim`, a finished row that must
never be rewrapped, such as a line of code; or `Gap`, a break between two
blocks. A `Gap` records the intention to separate, not a fixed number of
blank rows, because how much air a break gets is a layout decision rather
than a parsing one.

Second, that list is laid out. The `wrap` module takes the blocks produced
by parsing and fits them to a specific column width, one word at a time.
This is also where spacing happens, and it applies two different numbers:
one for the blank rows inside a wrapped paragraph, and a larger one for the
rows between paragraphs. Those have to differ. If the gap within a
paragraph matched the gap between paragraphs, every line would read as its
own paragraph and the page would lose all of its structure. The result is a
ratatui `Text`, a plain list of terminal rows ready to be shown.

A word, in this module, is not the same thing as a span. pulldown-cmark
emits smart punctuation as its own event, so `country's` arrives as three
separate spans: `country`, then the apostrophe, then `s`. Wrapping each
span independently and rejoining the results with spaces would render that
as `country ’ s`. The wrapper therefore defines a word as whatever sits
between two runs of whitespace, however many spans and styles it crosses.

Third, that `Text` is rendered. The `ui` module hands it to a ratatui
`Paragraph` widget, which draws it into a region of the screen, scrolled to
whichever page is currently in view.

The reason parsing and layout are two separate steps, rather than one, is
that the column width is not known until the terminal is actually being
drawn. The sidebar can be resized, the terminal window can be resized, and
a two-page spread uses a narrower column than a single page does. If
wrapping happened once, at parse time, none of that could be handled
without reparsing the whole document. Instead, `wrap::layout` runs fresh
every frame, against whatever width the current layout calls for, so a
resized terminal reflows correctly without any special handling anywhere
else in the app.

## Module map

**`theme`** holds the color palettes and nothing else. A `Theme` is plain
data, with no logic, depending on nothing but ratatui's color type. The
available palettes live in a `THEMES` static, which is a `static` rather
than a `const` so that `&THEMES[i]` borrows for `'static`. That detail
saves every other module from threading a lifetime parameter through
itself just to hold a reference to the current palette.

**`browser`** knows how to list a directory's contents and how to tell a
markdown file from any other kind of file. It has no idea that an `App` or
a terminal exists. Given a path, it returns a sorted list of entries. This
separation means the filesystem logic could be tested, or reused in a
completely different interface, without touching anything else.

**`markdown`** is the parsing stage described above. It depends on `theme`,
for colors, and on pulldown-cmark, for the actual markdown grammar. Its
only public output is `render_markdown`, which takes a string and a theme
and returns a `Parsed`: the `Vec<RenderLine>` to lay out, plus a flat list
of the document's headings for the table of contents. Both come from one
pass, so the contents list can never drift out of step with the blocks it
points into. Each heading records the index of the block that holds its
text, which is the handle the sidebar later uses to jump to it. Colors are
baked into the spans here, at parse time, which is why switching themes
reparses the open document. That is cheaper than carrying a semantic role on
every span and resolving it against the palette on every frame, and
documents are small enough that the reparse is not noticeable.

**`epub`** is the other way a document gets parsed. It opens an EPUB
container with the `epub` crate, walks every chapter in the book's reading
order, and converts each chapter's XHTML into the same `RenderLine` blocks
the markdown parser produces, using quick-xml for the markup. It ends at
exactly the same type `markdown` does, a `Parsed`, which is the point:
nothing downstream of parsing knows whether it is showing a note or a novel.
Chapter headings found in the content feed the table of contents, and when a
book carries no usable headings, the book's own navigation file is resolved
against each chapter's starting block and used instead. Inline styling is
honored both as semantic tags, `<em>` and `<strong>`, and as the classed
spans Calibre conversions emit, `<span class="italic">`, because a book that
loses its italics has quietly lost part of its text.

**`wrap`** is the layout stage described above. It depends on `markdown`,
for the `RenderLine` type it consumes, and on the `unicode-width` crate, to
measure how many terminal columns each word actually occupies. Its only
public output is `layout`, which takes a slice of blocks, a width, and a
`Spacing`, and returns a `Laid`: the ratatui `Text` to draw, plus a
`block_rows` vector giving the row each input block starts on. That second
value is what turns a heading's block index into a page: divide its row by
the viewport height and you have the page it falls on. Because layout runs
every frame at the current width, this mapping is always correct for the
width on screen right now.

**`session`** knows how to read and write the small file that remembers, between
runs, the last document opened, the page reached in every document seen so
far, and the typographic settings in force. Like `theme` and `browser`, it
depends on nothing above it and has no idea an `App` exists. Its format is a
plain, line-based text file rather than JSON, so it needs no serialization
dependency and stays readable on its own; an unrecognized line is skipped
rather than treated as an error, so a field added by a future version does no
harm to an older one.

**`app`** defines the `App` struct, which is the single source of truth for
everything the program currently knows: which directory the sidebar is
showing, which file is open, its headings, which page is on screen, and
which pane has keyboard focus. It also defines the methods that are allowed
to change several of those fields together, such as `load_file` and
`enter_dir`, so that related state always changes as a unit. `load_file`, for
instance, records the previous file's page before switching, then opens the
new file at whatever page was remembered for it, so moving between documents
resumes each one rather than restarting it. `app` depends on `browser`,
`markdown`, and `session`, since those are the modules that produce the data
an `App` holds.

**`events`** turns keyboard input into calls against an `App`. It reads
one crossterm event at a time and decides what should happen: move the
sidebar selection, turn a page, switch focus, or quit. It depends on `app`,
to mutate state, and on `browser`, to check whether a selected file is a
markdown file worth opening. Nothing in this module touches the terminal
directly.

**`ui`** is the only module that draws. Every function in it takes a
`Frame` and the current `App` and renders some part of the screen: the
sidebar, the reading pane, or the status bar. It depends on `app`, to read
state, on `browser`, for the same markdown check the sidebar uses to color
file names, on `theme`, for colors, and on `wrap`, to lay out the document
at whatever width its pane ends up being.

**`main`** is the thin entry point. It parses the one optional command line
argument, builds the initial `App`, and runs the event loop: draw a frame,
handle one event, repeat, until `App::quit` becomes true. It contains no
markdown logic, no drawing logic, and no key handling logic of its own.
Everything it does is delegate to the modules above.

A dependency runs in one direction only. `theme` and `session` depend on
nothing. `browser`, `markdown`, and `epub` depend only on `theme`, plus, for
`epub`, the `markdown` module's block types, since both parsers meet at the
same output. `wrap` depends on `markdown`. `app` depends on `browser`,
`markdown`, `epub`, and `session`. `events` and `ui` both depend on `app`,
plus whatever lower-level module they need directly. `main` depends on
everything. Nothing lower in this list ever depends on something higher,
which is what makes each module possible to read in isolation.

## The App struct as the single source of truth

booknook draws in immediate mode. There is no long-lived widget tree and no
incremental updates. Every single frame, `ui::draw` reads the current
`App` from scratch and renders the entire screen again, sidebar, reading
pane, and status bar included. This is simpler to reason about than a
retained UI, at the cost of doing more work per frame, and for a terminal
reader showing a few dozen visible rows, that cost does not matter.

Because rendering is stateless, all the state that matters lives in one
place. If a value affects what gets drawn, it belongs on `App`. A
consequence of this is that a few fields exist purely to let two otherwise
separate steps talk to each other across a frame boundary. The clearest
example is `spread`: `ui::draw_document` is the only code that knows whether
the terminal is currently wide enough for a two-page layout, but
`events::handle_document_key` needs that same fact to decide whether a page
turn should move by one page or by two. Rather than recomputing the width
check in the event handler, `draw_document` writes the answer into `App`
every frame, and the event handler reads it back on the next keypress. This
is a deliberate exception to the general rule that state flows one
direction, from input to app to render, and it is called out in the field's
own comment so it does not look accidental later.

Two more fields work the same way, both in service of the table of contents.
`pending_jump` carries a request in the other direction, from input toward
render: choosing a heading sets it to that heading's block index, and the
next draw resolves the block to a page and clears it, because only the draw
step knows the viewport height that turns a row into a page. `active_heading`
carries an answer back the same way `spread` does: each draw works out which
heading the visible page falls under and records it, so the sidebar can
highlight that entry. Both are called out in their own comments for the same
reason `spread` is.

## Pages, not scroll rows

The reading pane stores `page: u16`, a page number, rather than a scroll
offset in rows. This choice is what makes booknook behave like an e-ink
reader instead of a text pager. A row offset is a derived quantity: it
depends on how tall the current viewport is, which changes if the terminal
is resized. A page number is the real thing being tracked. Turning a page
increments or decrements that number, and only at draw time does `ui`
convert it into an actual row offset, by multiplying the page number by the
viewport height. If the terminal is resized between two frames, the app
does not end up scrolled to a strange half-page position. It reflows and
lands back on the same page number, wherever that page's content now
starts.

The multiplication only works because every page is exactly one viewport
tall, and that is now a promise `wrap::paginate` keeps rather than an
accident of arithmetic. After layout produces the document's rows, each
tagged with whether a page may end after it, pagination walks them page by
page and moves any break that would land badly: a heading at a page
bottom, a paragraph's first line stranded there, its last line alone at
the top of the next page, a table row cut through a wrapped cell. The page
ends a line or two early instead, padded with blank rows out to the
viewport, the way a typesetter leaves a short page rather than a bad
break. The padding keeps the page-to-row conversion a plain multiply while
letting the breaks themselves be chosen.

Because the true number of pages is not known until layout has happened,
`events::handle_document_key` sometimes asks for a page that does not
exist yet. Jumping to the last page, bound to `G`, sets `page` to `u16::MAX`
rather than trying to compute the real last page number itself. The one
place that does know the real bound, `ui::draw_document` and the functions
it calls, clamps whatever value it is given down to something that
actually exists. This keeps the knowledge of page bounds in a single
place, rather than duplicating that computation in the key handler.

## The two-page spread

When the reading pane is wide enough, booknook shows two pages side by
side with a vertical rule between them, standing in for a book's spine.
Both halves are produced from one call to `wrap::layout`, at the same
column width, so that continuing from the left page to the right page
reads exactly like turning past the middle of an open book rather than
reflowing into a different shape.

The left page always shows an even-numbered page, the same way a real
book's left-hand pages are always even. `ui::draw_spread` enforces this by
rounding `app.page` down by one whenever it is odd, every frame, and
`events::handle_document_key` steps by two pages at a time whenever
`app.spread` is true. Neither function needs to know about the other's
half of this rule. Each one keeps its own part correct, and the two stay in
sync as a result.

If the terminal is not wide enough for two comfortable columns, the same
frame that would have drawn a spread draws a single page instead. That
decision is remade every frame, based on the current width, so resizing a
terminal window between narrow and wide switches modes immediately, with
no toggle or setting involved.

## The table of contents

The sidebar shows two stacked lists: the file browser on top and, once a
document with headings is open, its table of contents below. The contents
list is a navigation aid and a "you are here" marker at once, and making it
work threads a single fact, a heading's position, through three modules
without any of them having to know the whole story.

Parsing is where a heading becomes trackable. As `markdown::render_markdown`
walks the event stream, each heading it closes is recorded as a `Heading`
carrying its level, its text, and the index of the block that holds it. A
block index is the right handle to keep, rather than a row or a page, because
it survives reflow. The row a heading sits on changes every time the column
width changes; which block it is does not.

Turning that block index into a page is a job for layout, and only at draw
time. `wrap::layout` already walks every block to produce the page, so it
records, as it goes, the row each block starts on, and returns that alongside
the text. To jump to a heading, `ui` looks up its block in that table, divides
the row by the viewport height, and lands on the page. The same table, read
the other way, answers the reverse question: which heading is the current page
under. That is just the last heading whose row has already scrolled into or
above the visible window, and because headings and their rows both run in
document order, the search stops at the first heading that has not appeared
yet.

The event handler, in the middle, knows none of this. Choosing a heading only
sets `pending_jump` to its block index and hands focus to the reader. It never
computes a page, because at the moment a key is pressed the viewport height
that a page depends on is not its to know. The draw step owns that knowledge,
so the draw step does the conversion, on the next frame, and clears the
request. This is the same division of labor that lets `G` ask for the last
page without computing it: the key handler states an intention, and the one
place that knows the true bounds resolves it.

## Remembering the reading position

booknook reopens a document on the page it was left on, and launched with no
argument it reopens the last file entirely, the way a Kindle returns to the
book you closed. All of that lives in the `session` module and a handful of
fields on `App`, and the shape of it follows one decision: the position is
remembered per file, not once globally, because a reader expects to return to
the middle of a long essay it left yesterday even after dipping into three
other files since.

So `App` holds a map from a file's path to the page reached in it. The map is
loaded from the saved session at startup and written back on quit, and only
the open file's entry changes while running. Two moments update it: opening a
different file, which records the outgoing file's page before switching, and
quitting, which records the current file's page before saving. Between those,
the live page number is enough; there is no need to touch the map on every
page turn.

The keys in that map are canonical paths, resolved through
`fs::canonicalize`, so the same file reached by a relative path, an absolute
one, or a symlink all land on one entry rather than three. A path that cannot
be canonicalized, because the file has since moved, falls back to its own
form, which is still a consistent key for the life of the process.

The state file itself is deliberately plain text, one `key\tvalue` line at a
time, not JSON. That keeps `session` free of any serialization dependency,
keeps the file readable and hand-editable, and makes forward compatibility
trivial: a line whose key the parser does not recognize is skipped, so an
older build reading a file written by a newer one simply ignores the fields
it does not understand rather than failing to load. Reading and writing are
split into a pure parse-and-serialize pair with the file I/O wrapped around
them, which is what lets the format be tested by round-tripping a `Session`
through a string with no filesystem involved.

## Ownership and the borrow checker in practice

booknook was also built as a way to learn Rust by writing it, and its
ownership choices are worth walking through on their own, since they show
up as concrete decisions rather than abstract rules.

**Owned data outlives the parser that produced it.** pulldown-cmark hands
`markdown::render_markdown` borrowed text, in the form of a `CowStr` tied to
the lifetime of the original markdown string. Every span kept in a
`RenderLine`, though, is built with `Span::styled(text.into_string(),
style)`, which copies that borrowed text into an owned `String`. This is
what lets `Vec<RenderLine>` carry the `'static` lifetime it needs to live on
`App` as `blocks`. If the spans still borrowed from the source string, the
parsed document could not outlive the local variable holding the raw file
contents inside `load_file`, and `App` would need a lifetime parameter of
its own just to hold a parsed document. Paying for a handful of string
copies once, at load time, avoids that entirely.

**`mem::take` moves a value out of a mutable reference.** Rust will not let
you move a value out of a `&mut Vec<T>` and leave the original variable
behind in an undefined state; the compiler has no way to know what should
be there afterward. `std::mem::take` is the escape hatch: it swaps in a
fresh, empty `Vec` and returns the old one, fully owned. Both
`markdown::flush_prose` and the word-wrap loop in `wrap::wrap_prose` use
this to hand off a batch of spans, `std::mem::take(spans)` and
`std::mem::take(&mut current)`, without needing to clone anything or
restructure the surrounding loop.

**Borrowing decides a function's signature, not just its body.** Every
function in `ui` that only reads state, such as `draw_sidebar` and
`draw_status_bar`, takes `app: &App`. The handful that also need to clamp
`app.page` once the true page count is known, such as `draw_document` and
`draw_spread`, take `app: &mut App` instead. This is not an arbitrary
choice enforced after the fact. A shared reference simply does not compile
against code that assigns to a field, so the signature itself is a true
statement about what the function can do, readable without looking at the
body at all.

**Methods keep a single mutable borrow instead of many.** `App::load_file`
and `App::enter_dir` take `&mut self` and write several related fields
inside one method body. Before this existed as methods, the equivalent free
functions took `app: &mut App` as a parameter, which works the same way at
the borrow-checker level, but grouping the writes as methods on `App`
itself keeps the invariant, that `dir` and `entries` always change
together, or that loading a file always resets `page` to zero, defined in
exactly one place rather than trusted to every call site.

**Cloning is sometimes the correct tool, not a workaround.** `ui::draw_spread`
calls `wrap::layout` once and then writes `Paragraph::new(text.clone())` for
the left page before writing `Paragraph::new(text)` for the right. ratatui's
widgets are consumed by the methods that configure them, so `.scroll(...)`
takes `self` by value. Two independent widgets, scrolled to two different
page offsets, need two independent copies of the underlying `Text` to
consume, since the first `Paragraph::new` call would otherwise move the
only copy that exists. The clone here is not covering up a design problem.
It is the honest cost of needing the same data to exist twice at once.

**The smallest possible clone breaks a borrow conflict.** In
`events::handle_sidebar_key`, opening the selected entry looks like this:

```rust
if let Some(entry) = app.entries.get(app.selected) {
    if entry.is_dir {
        let target = entry.path.clone();
        app.enter_dir(target);
    }
    // ...
}
```

`entry` is a shared reference borrowed from `app.entries`, which is itself
part of `app`. Calling `app.enter_dir(...)` needs a mutable borrow of the
whole `App`, which cannot coexist with a live shared borrow of one of its
fields. Cloning `entry.path` into an owned `PathBuf` first, called `target`,
ends the dependency on `entry` before `app.enter_dir` is called. Under
Rust's non-lexical lifetimes, a borrow's lifetime ends at its last actual
use, not at the end of the block it was created in, so the borrow of
`app.entries` through `entry` is already over by the time `target` is used,
and the compiler accepts the mutable borrow that follows without any extra
scoping. This pattern, clone the one small piece of data that truly needs
to survive, then let the original borrow end, comes up often enough in this
codebase that it is worth recognizing on sight rather than re-deriving each
time.

## Extending booknook

A new inline markdown feature, such as strikethrough or footnotes, belongs
in `markdown::render_markdown`. Most of these follow the same shape as the
existing handlers: push a style onto `style_stack` on the matching `Start`
event, and pop it back off on the matching `End` event.

A new block-level feature, such as tables, is more work, because it has to
decide how that block behaves under `wrap::layout`. Something that should
reflow with the rest of the paragraph belongs in a `RenderLine::Prose`
block, whose `indent` and `hang` fields control where its first row and its
continuation rows begin. Note that the word splitter discards leading
whitespace, so an indent has to travel in that `indent` field rather than
as spaces baked into the text. Something with its own fixed shape, the way
a code block does, belongs in one or more `RenderLine::Verbatim` lines
instead. A separation between blocks is a `RenderLine::Gap`, which lets
`wrap` decide how many rows it is actually worth.

A new color palette is an entry appended to the `THEMES` static in `theme`,
and nothing else. Nothing needs to change anywhere else, because `t` cycles
by index over whatever is in that array.

A whole new pane, alongside the sidebar and the reader, would touch four
modules. `app` would need new state for whatever that pane shows. `events`
would need a new key handler, and a way to route keys to it when it has
focus. `ui` would need a function that draws it, wired into `draw`'s
layout split. `main` would not need to change at all, since it only wires
together whatever `app`, `events`, and `ui` already expose.
