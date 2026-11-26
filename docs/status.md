# Status

Newest first.

## 2026-11-26: measured on the Intel Mac; scrolling is inside the frame budget

macOS 15.7 on a 2.6 GHz six-core Intel laptop, release build.

- All 53 tests pass there, on the default backend and on the faster one.
- `frames`, 2,496 cells a frame, the grid's share before the renderer. Three runs on a quiet machine (load average under 2), and three more at the latest commit with the machine busier (7 to 11) that agree with them:

| | default backend | faster backend |
|---|---|---|
| Scrolling three rows a frame | 5.4 to 5.8 ms (99th percentile 8.5 to 10.9 ms) | 5.2 ms (8.7 to 10.3 ms) |
| A whole new screen every frame | 7.0 to 7.2 ms | 4.6 ms |
| A screen a frame, back and forth | 3.5 to 3.8 ms | 4.5 to 4.6 ms |

**What this means.** Milestone 0 asks for a frame under 6 ms at 2,500 visible cells while scrolling, with more than 95% of text from the cache. Scrolling three rows a frame serves 97.6% from the cache and takes 5.4 to 5.8 ms on the default backend and 5.2 ms on the faster one. **That part of the budget is met**, on the code that took 8.4 ms at 1,950 cells yesterday. The margin on the default backend is thin.

**What is over.** A scrollbar drag that shows a whole new screen every frame takes 7.0 to 7.2 ms on the default backend, because shaping the 256 new lines a frame allows costs more on this machine than on Linux. It is 4.6 ms on the faster backend. Lowering the number of new lines a frame would bring it under, at the price of text arriving later after a jump; that has not been done.

**What cannot be claimed yet.** The budget is written for a frame with the semantic shadow on. The shadow is not built, so its cost is not in these numbers.

**Not measured:** the renderer's own time, a real display. Nobody has used the playground by hand on the Mac, where the column menu, the search bar and flipping a yes-or-no cell are new since the last time it was opened there. The frames were not rendered to images there again after the paint change; on Linux they are identical to the pixel.

## 2026-11-26: a space flips a yes-or-no cell

- Space on a cell of a yes-or-no column flips it instead of starting to type: true becomes false, and false or an empty cell becomes true. It is one edit through the source, so undo takes it back. In any other column a space is typed as before. `GridState::toggle_cell` does it without the key.
- A test flips a false cell twice, fills an empty one and undoes it, and types a space into a text cell. Both backends.

## 2026-11-26: a search bar

- Command-F opens a search bar over the top right of the cells. Typing shows only the rows that hold the text in some text column, ignoring case and accents, and the bar says how many rows that is. Enter goes to the rows found and keeps the search; Escape in the bar closes it and shows every row again (`docs/keys.md`).
- The search is the query's own `search` text, which the pipeline already ran, so sorting, grouping and pins apply to what is found. `GridState::set_search` sets it without the bar.
- The bar sits beside the cells in the view, not inside them, so keys typed in it are not also the grid's. A test types in it, presses the grid's own keys there, and checks the rows and that no cell changed, on both backends.
- Tried in a real window on Linux on the playground's 100,000 rows: "zurich" finds the 14,225 rows in Zürich. Not tried on the Mac.

## 2026-11-26: a menu for columns and cells

- A secondary click on a column title or a cell opens the component library's context menu: sort ascending or descending (the current one is checked), clear the sort, pin the columns up to this one, unpin, fit the column to its text, hide it, show hidden columns, cut, copy and paste. Entries that would do nothing are disabled. A secondary click on a cell outside the selection selects it first; inside the selection it leaves the selection alone.
- Every entry is an action the view answers, so a host can bind keys to them or use them from its own menus (`docs/keys.md`). `GridState` gained `sort`, `sorted` and `hidden_columns`.
- Checked in a headless window by a test that opens the menu and runs each action, on both backends, and in a real window on Linux: the menu opens under the pointer and choosing "Sort descending" sorts the column and closes it. Not tried on the Mac.

## 2026-11-26: a frame at the budget's size takes half the time

Three changes to how cells are painted, all in this repository. The entry of 25 November said nothing left in this repository's code would close the gap to the frame budget. **That was wrong:** two of the three things below were this code's own waste.

- **Shaped lines are shared, not copied.** A shaped line is a few kilobytes wide, most of it room for decorations a cell never has. The cache held them by value, so every growth of its table moved megabytes, and it handed out a copy for every cell painted. It now holds and hands out a pointer. This is what the long frames were: the 99th percentile fell from 18 ms to 7 ms.
- **A band's text goes into one layer.** The library paints each line of text inside a layer of its own, and finding each layer's place among what is already painted was about a third of a frame at 2,500 lines. The cells of a band (the scrolling cells, each pinned band, the titles) do not overlap, so the grid opens one layer for a band and paints its glyphs into it. The plain and grouped frames come out identical to the pixel; in the pinned frame 15 pixels differ by one level in 2.2 million.
- **A cell drawn again keeps its line.** Lines are also kept by row ID and column for one version of the table's data, so a cell that was on the screen a moment ago is not read, formatted and looked up by its text again. An edit starts a new version; the lines kept by text stay, so nothing is shaped again. A test reads back what each cell was drawn with after an edit, an undo, a redo and a sort.

`frames`, 2,496 cells a frame, the grid's share before the renderer, two-core Linux machine, three runs of 290 frames:

| | default backend, before | default backend, now | faster backend, now |
|---|---|---|---|
| Scrolling three rows a frame | 7.8 ms | 3.7 to 3.9 ms | 4.1 to 4.3 ms |
| A whole new screen every frame | 5.9 ms | 4.6 to 5.1 ms | 4.0 to 4.2 ms |
| A screen a frame, back and forth | 6.9 ms | 2.8 to 3.0 ms | 3.5 to 3.6 ms |
| 99th percentile, scrolling | 18 ms | 6.2 to 7.9 ms | 6.2 to 8.6 ms |

The default backend is now as fast as the faster one, because painting glyphs one by one gives up that backend's own way of painting a run, which was most of its lead. The budget is under 6 ms on the Intel Mac, which ran the earlier code about 1.3 times slower than this machine; by that ratio scrolling would be about 5 ms there. **That is an estimate. It has not been measured on the Mac:** a first attempt ran while the machine's load average was between 20 and 100 and its numbers mean nothing.

The fallback named in the plan, rows kept as retained elements, is not built and on these numbers is not needed.

`frames` takes the number of rows and frames as arguments.

## 2026-11-26: builds on the optional faster backend

- `grid-gpui` has a `fast` feature that builds the view on the component library's optional faster backend. It is off by default, and a host turns it on for its whole build or not at all, so the editor has the same feature.
- All 49 tests pass on both backends. The shared CI job builds with every feature, which is the faster backend; the headless job builds the default one, so every pull request is checked on both.
- Three tests leaned on every frame being built from scratch, which the faster backend does not do: it draws a view again from its last frame unless the view was told something changed. They now tell the view, as a running window does when it asks for another frame, and move the clock on for the scrollbar, which redraws a drag at most so many times a second.
- `frames` now draws 2,496 cells, the size the budget is set for (it drew 1,950). On the two-core Linux machine, mean of 290 frames:

| | default backend | faster backend |
|---|---|---|
| Scrolling three rows a frame | 7.8 ms | 5.4 ms |
| A whole new screen every frame | 5.9 ms | 4.3 ms |
| A screen a frame, back and forth | 6.9 ms | 4.6 ms |

The faster backend takes about 30% off a frame as the grid stood. This is the backend alone, with the grid still painting every visible cell each frame; rows kept as retained elements are not built. Not measured on the Mac.

Earlier entries: [`status-2026-11-25.md`](status-2026-11-25.md).
