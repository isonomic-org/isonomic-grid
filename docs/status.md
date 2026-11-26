# Status

Newest first.

## 2026-11-26: builds on the optional faster backend

- `grid-gpui` has a `fast` feature that builds the view on the component library's optional faster backend. It is off by default, and a host turns it on for its whole build or not at all, so the editor has the same feature.
- All 49 tests pass on both backends. The shared CI job builds with every feature, which is the faster backend; the headless job builds the default one, so every pull request is checked on both.
- Three tests leaned on every frame being built from scratch, which the faster backend does not do: it draws a view again from its last frame unless the view was told something changed. They now tell the view, as a running window does when it asks for another frame, and move the clock on for the scrollbar, which redraws a drag at most so many times a second.
- `frames` now draws 2,496 cells, the size the budget is set for (it drew 1,950), and takes the number of rows and frames as arguments. On the two-core Linux machine, mean of 290 frames:

| | default backend | faster backend |
|---|---|---|
| Scrolling three rows a frame | 7.8 ms | 5.4 ms |
| A whole new screen every frame | 5.9 ms | 4.3 ms |
| A screen a frame, back and forth | 6.9 ms | 4.6 ms |

The faster backend takes about 30% off a frame as the grid stood. This is the backend alone, with the grid still painting every visible cell each frame; rows kept as retained elements are not built. Not measured on the Mac.

## 2026-11-25: measured on the Intel Mac; the frame budget is not met at the stress size

macOS 15.7 on a 2.6 GHz six-core Intel laptop, native renderer, release build, three runs that agreed to within 0.3 ms. The machine was not idle (load average 7 to 12).

- All 49 tests pass there, and the `screenshot` example renders the plain, grouped and pinned frames.
- `frames`, 1,950 cells a frame, the grid's share before the renderer:

| | mean | 99th percentile |
|---|---|---|
| Scrolling three rows a frame | 8.3 to 8.5 ms | 14 ms |
| A whole new screen every frame | 8.3 to 8.8 ms | 11 to 15 ms |
| A screen a frame, back and forth (every line cached) | 6.4 to 6.6 ms | 7 ms |

**What this means.** Milestone 0 asks for under 6 ms at 2,500 cells on this machine. With every line of text already shaped, 1,950 cells take 6.5 ms, about 3.3 µs a cell, which is 8 ms or more at 2,500. The time is the toolkit painting glyphs, one sprite lookup each, roughly 12,000 a frame; this grid's own work (reading cells, layout, the cache) is under a millisecond of it. Nothing left in this repository's code will close that gap.

**What is not decided.** The specification names the fallback: rows kept as retained elements on the optional faster backend. That is a change of rendering path, not a tuning, and it has not been started. A grid of ordinary size should be far inside the budget: at the same cost a cell, 400 cells work out to about 1.5 ms. That size has not been measured.

**Not measured:** a quiet machine, the renderer's own time, a real display.

## 2026-11-25: keys for the ends of a column and whole rows and columns

- The command key with an arrow goes to the first or last row of the column or the first or last column of the row; with Shift it extends the range there. Shift with Home or End extends along the row.
- Shift with Space selects the active cell's row, Control with Space its column; in a cell being edited they type a space.
- `docs/keys.md` lists every key and what the pointer does.
- A test presses every key, clicks inside and outside, and calls every column and pin function on a grid with no rows, with and without columns. It found one thing: a paste into a grid with no columns added empty rows; it now does nothing.

## 2026-11-25: columns move, hide, pin and fit

- A press on a column's title that becomes a drag moves the column, live, to wherever the pointer is; a press that does not still sorts. `move_column`, `hide_column` and `show_column` do the same from code, and the selection, held by column ID, goes with its columns.
- `pin_columns` and `pin_rows` pin any number at each end. Pinned cells are drawn over what scrolls beneath them, with a line along the band's inner edge, and the active cell's outline is cut off where it passes under one. `docs/images/grid-pinned.png` is a rendered frame. There is no control in the grid itself for pinning yet; it is the host's to call.
- A double click on a column's edge sizes it to the widest text it shows now, its title included.
- Found and fixed on the way: with a pinned column, the titles of scrolled columns were painted over the pinned column's title, and every pinned cell was drawn in the title color.

## 2026-11-25: the selection is held by row and column ID

- The keyboard and the pointer work in positions; after every change to the selection the state notes the row IDs and column IDs under the active cell and under each range's two corners. After a sort, a filter, a group opening or an edit, it puts the selection back on those rows. This is rule 2, which the view had not been keeping: a sort used to leave the selection on whatever rows landed in the old positions.
- A range keeps whichever corners still exist; a selection whose rows are all gone falls back to the cell now at the old position. Select-all stays select-all whatever the rows become.
- `docs/requests.md` lists what this repository needs from the component library and the SDK. None of it has been filed in those repositories yet.

## 2026-11-25: dragging selects a range

- Pressing on a cell and dragging extends the selection to the cell under the pointer. With the pointer held past an edge the grid scrolls that way, faster the further out it is, and the range grows with every frame until the button is released.

## 2026-11-25: a frame shapes only so much new text

- One frame shapes at most 256 lines of text it has never seen. A jump to rows never shown draws what it can, leaves the other cells empty, and fills them in over the next frames. On the same machine and run as the table below, a whole new screen every frame went from 19.7 ms to 5.4 ms a frame; steady scrolling is unchanged.
- The view counts the most lines any one frame shaped, and a test holds it to the limit.
- On the Intel Mac (2.6 GHz, six cores) the `frames` example ran, but while the machine was busy with other builds (load average above 13): 9 to 11 ms a frame scrolling three rows, before this change. That is not a clean number; it needs a rerun on a quiet machine before it says anything about the 6 ms budget.

## 2026-11-25: the cell editor has a caret; paste, cut and range clear

- The line being edited is a headless type in `grid-interact`: text, a caret and the part an input method is composing, with offsets that never split a character.
- Double click, F2 or Enter opens a cell with its own text; then Left, Right, Home and End move the caret, Delete removes forward, and long text slides to keep the caret in the cell. An edit begun by typing still confirms and moves on an arrow key.
- Input methods compose in place, underlined, through the platform text input interface.
- Paste puts tab-separated text on the active cell and down and right from it, stepping over group rows; one value pasted over a range fills the range; rows past the last become new rows; text a column cannot hold refuses the whole paste and the state emits why. Cut copies and clears. Delete clears every editable cell in the selection. Each is one undo step, and each is limited to 200,000 cells.
- The pointer shows a column-resize shape over a header edge.

## 2026-11-25: scrollbars, a shaped-line cache and frame timings

- The component library's standard scrollbars sit over the grid, read a handle that reports each axis scaled into a range they hold exactly, and write a dragged thumb back as a 64-bit offset. An axis nobody dragged is left exactly where it was. Tested by dragging the vertical thumb across one million rows: halfway is the middle, and past the end is the last row.
- Cell text is shaped once and kept in a bounded two-generation cache keyed by the text, on top of the toolkit's own cache of the previous frame. Text is clipped only when it is wider than its cell. Each row has one rule under it instead of one per cell.
- The active cell has an outline.
- `cargo run --release -p grid-gpui --example frames` times the grid's own share of a frame (reading the visible cells, laying them out, painting them into a scene; not the renderer drawing that scene) on 100,000 rows by 50 columns in a 2560 by 1440 window, 1,950 cells a frame.

| 2-core 2.1 GHz Linux machine | mean | 99th percentile | text from the cache |
|---|---|---|---|
| Scrolling three rows a frame | 5.5 ms | 12.6 ms | 97.5% |
| A whole new screen every frame | 19.7 ms | 28.9 ms | 58.7% |
| A screen a frame, back and forth | 5.5 ms | 12.2 ms | 99.7% |

Before these changes the first line was 7.5 ms. Of what remains, about 3.3 ms is the toolkit painting glyphs, one sprite lookup each; a whole new screen adds about 4.5 ms of shaping text never seen before. See the entry above for the first, unclean, macOS run.

## 2026-11-25: the grid draws and takes input

`ui` re-exports the component library at one pinned revision. `grid-gpui` adds:

- `GridState`: an in-memory source, the view query, visible columns and widths, the selection, the scroll position and the edit log.
- `GridView`: one element paints the visible cells and the header and nothing else; a click on a header sorts, a click on a group row opens or closes it; the pointer and the keyboard move and extend the selection and scroll it into view; the wheel scrolls in 64-bit offsets; copy writes tab-separated text; undo and redo go through the edit log.

Tested in a headless window with real layout and real events: a 100,000-row grid paints one screenful of cells at the top and at the very end; pointer and keyboard selection with paging; sort and group toggle from clicks; edit, copy, undo.

**Not yet, and known rough edges.**

1. It has been rendered offscreen with the real renderer on Linux (software Vulkan); the frames are in `docs/images/`, made by the `screenshot` example. The `playground` example opens it in a window; it was run on Linux under a virtual display with software rendering and driven with real pointer and key events (click, type, shortcuts, wheel). On macOS (Intel, macOS 15) every test passes and the `screenshot` example renders its frames with the native renderer. Nobody has used it by hand yet.
2. Cells edit in place; see the entry above. The editor is one line with a caret and no selection of its own, and there are no pickers for dates or choices yet. A refused paste is reported as an event, which nothing shows yet.
3. A frame of entirely new text fills in over a few frames; see the entries above.
4. A column's right edge in the header drags to resize it, down to a minimum width, and the pointer shows it. The standard scrollbars work. Columns move by dragging their titles and fit their contents on a double click of an edge. Pins work from code; the grid has no menu for them, or for hiding columns, yet.
5. The view takes the in-memory source only; windowed sources come with the engine integration.
6. No semantic shadow for assistive technology yet.

## 2026-11-25: text is dictionary-encoded; sorting is a stable radix sort

- Text columns keep each distinct string once and a code per row. The dictionary's folded sort order is computed once and reused until a new string arrives. Filters and search test each distinct string once.
- Sorting runs a stable byte-wise radix sort per key, from the least significant key to the most, and skips byte positions where every key agrees.
- Negative zero now sorts equal to zero, as comparison already said. A new test checks the fast path against a plain comparison sort on 6,000 rows, including nulls, not-a-number, descending keys and a restored row.

One million rows, same machine as below; each figure includes about 50 ms to emit one million result rows:

| | Before | Now |
|---|---|---|
| Sort by an integer column | 136 ms | 118 ms |
| Sort by two keys | 222 ms | 163 ms |
| Sort by a mostly distinct text column | 1.24 s | 0.63 s first time, then cached |
| Search across all columns | 75 ms | 32 ms |
| Group by a text column with sum and average | 181 ms | 73 ms |

## 2026-11-25: licence

The repository is licensed under GPL-3.0-or-later: LICENSE at the root, the licence field in every manifest, and the copyright and SPDX lines at the top of every Rust source file. SECURITY.md says how to report a vulnerability. No behaviour changed.

## 2026-11-25: version 0.1, the headless engine

**What exists.** Five headless crates, tested, with no GPU or window needed.

| Crate | State |
|---|---|
| `grid-core` | Fifteen value kinds, typed columnar storage with validity, stable row IDs, limits, the serializable view query, the `DataSource` trait with paging by range or by key. |
| `grid-query` | Filter (typed loops over bitmaps), search, multi-key sort on packed keys, nested grouping, six aggregates, expansion, flattening; `MemorySource` with a cached result and all-or-nothing edits. |
| `grid-layout` | Uniform and variable axes (prefix sums), three bands per axis for pinned regions, hit testing, reveal, scroll anchors, a scaled scrollbar range. |
| `grid-interact` | Active cell and ranges, twelve keyboard moves, pointer selection, row selection stored as "all except", undo and redo through inverse edits. |
| `grid-formats` | Delimited text both ways, parse and format for every value kind, strict UTC dates, import with type inference, paste to edits. |

**Tests.** 23, all hermetic: the shared semantics (null and not-a-number order, folded text order, ties by row ID), filters and search, grouping and aggregates, paging, atomic edits, axes against plain sums, pinned bands and hit testing, ten million rows, navigation, the edit log, format round trips.

**Measured** (release build, 2-core 2.1 GHz Linux machine, one million rows, `cargo run --release -p grid-query --example bench`):

| | Time |
|---|---|
| Numeric filter | 3.8 ms |
| Text contains | 30 ms |
| Search across all columns | 75 ms |
| Sort by an integer column | 136 ms |
| Sort by two keys | 222 ms |
| Sort by a mostly distinct text column | 1.24 s |
| Group by a text column with sum and average | 181 ms |

Every figure includes about 40 ms to emit one million result rows.

**Not done, in the order it should be done.**

1. **Nothing is drawn yet.** `grid-gpui`, `ui` and the playground do not exist. The component library was confirmed to compile on Linux at revision `834fc6a5`. This is milestone 1's first task.
2. ~~Text columns are not dictionary-encoded in storage.~~ Done; see the entry above.
3. **Text order is folded, not collated.** Lower case with common Latin accents removed, then bytes. Locale-aware collation goes behind the same dictionary once the collation dependency is approved.
4. **Sorting emits every row.** A windowed result (top of the order only) and running the pipeline off the main thread are milestone 2.
5. **macOS runs the tests and renders frames; nothing has been measured there.** All numbers above are from Linux.
6. Pivot, tree data, computed columns, the columnar interchange adapter, typed clipboard payloads and the semantic shadow for assistive technology are later milestones.
7. Requests to other repositories (the SDK `grid` module reservation and the UI requests in the spec) are not filed.

**Decisions made while building, to confirm.**

- `DataSource` is synchronous in 0.1. The view crate will wrap slow sources in tasks; if the engine source needs an asynchronous trait, this is where it changes.
- Integers, dates, date-times, durations, choices and references share one integer vector; decimals, multi-choice, JSON, bytes and custom values use a general vector until they earn a typed one.
- A comparison between a text cell and a number (or the reverse) matches nothing rather than raising an error.
- Text equality in a filter is exact; contains and starts-with ignore case and accents.
