# Status

Newest first.

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

Before these changes the first line was 7.5 ms. Of what remains, about 3.3 ms is the toolkit painting glyphs, one sprite lookup each; a whole new screen adds about 4.5 ms of shaping text never seen before. Neither has been measured on macOS yet.

## 2026-11-25: the grid draws and takes input

`ui` re-exports the component library at one pinned revision. `grid-gpui` adds:

- `GridState`: an in-memory source, the view query, visible columns and widths, the selection, the scroll position and the edit log.
- `GridView`: one element paints the visible cells and the header and nothing else; a click on a header sorts, a click on a group row opens or closes it; the pointer and the keyboard move and extend the selection and scroll it into view; the wheel scrolls in 64-bit offsets; copy writes tab-separated text; undo and redo go through the edit log.

Tested in a headless window with real layout and real events: a 100,000-row grid paints one screenful of cells at the top and at the very end; pointer and keyboard selection with paging; sort and group toggle from clicks; edit, copy, undo.

**Not yet, and known rough edges.**

1. It has been rendered offscreen with the real renderer on Linux (software Vulkan); the frames are in `docs/images/`, made by the `screenshot` example. The `playground` example opens it in a window; it was run on Linux under a virtual display with software rendering and driven with real pointer and key events (click, type, shortcuts, wheel). On macOS (Intel, macOS 15) every test passes and the `screenshot` example renders its frames with the native renderer; the window itself has not been opened there, and nobody has used it by hand yet.
2. Cells edit in place: typing starts an edit, Enter confirms and moves down, Escape cancels, F2 amends, Delete clears, and text the column cannot hold is refused with the editor left open. The editor is a single line with the caret at the end; no composition, and no pickers for dates or choices yet.
3. A frame of entirely new text still shapes every cell; see the timings above.
4. A column's right edge in the header drags to resize it, down to a minimum width; the pointer does not change shape over the edge yet. The standard scrollbars work. There is no column reorder and there are no pin controls, though the layout crate already supports pins.
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
