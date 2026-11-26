# Status

Newest first.

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
