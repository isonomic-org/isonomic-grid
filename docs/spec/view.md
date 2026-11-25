# Isonomic Grid: Layout, rendering and interaction

## B7. Layout

- **Axes.** Rows and columns are each an axis: sizes, order, hidden items, pinned-start and pinned-end counts. Uniform sizes use arithmetic; variable sizes use a prefix-sum tree for O(log n) offset-to-index and index-to-offset.
- **Regions.** Pinned columns left and right, pinned rows top and bottom, giving nine regions that scroll together on the correct axis. Multi-level column headers, a filter row, and footer rows for totals sit outside the body.
- **Viewport.** Given scroll anchor and size, layout returns the visible ranges per region plus overscan. Hit testing is arithmetic over the same model: point to (region, row, column, zone), where zone is cell, resize edge, fill handle, group toggle or header menu.
- **Spans.** Full-width rows (group headers, detail rows) and merged cells are layout entries, not special cases in the painter.
- **Sizing.** Fixed, flexible (share of free space), fit-to-content (measured over a sample plus the visible window), with minimum and maximum. Double-click on a resize edge fits the column.
- **Row heights.** Fixed by density by default, which keeps every offset arithmetic. A view may switch to automatic heights for wrapped text: rows start from an estimate, are measured when they first become visible, and scroll anchoring keeps the anchor row fixed while heights above it settle.

## B8. Rendering

- **One element.** `Grid` paints each region: backgrounds, grid lines, then cell contents clipped to their bounds, then selection and focus overlays.
- **Text cache.** A shaped line is cached by (text, font, size) in a bounded least-recently-used cache; scrolling reuses shaped lines, and truncation is decided at paint from the cell width.
- **Scrolling.** The element handles wheel and trackpad input itself and keeps (anchor row, offset) in 64-bit for rows and a pixel offset for columns. It implements the UI library's scrollbar interface (viewport bounds, offset, content size) over a range scaled to fit 32-bit pixels, so the standard scrollbars, their dragging and their auto-hide work unchanged.
- **Painters.** Each column type paints its cells. Built-in painters cover text with truncation, numbers with alignment, checkboxes, choice pills, links, dates, progress bars and sparklines.

```rust
pub trait CellPainter: 'static {
    fn paint(&self, cell: &CellPaint<'_>, window: &mut Window, cx: &mut App);
    fn measure(&self, cell: &CellMeasure<'_>) -> Option<Size<Pixels>> { None }
    fn widget(&self, cell: &CellCx<'_>) -> Option<AnyElement> { None }   // live component, opt-in
}
```

The signatures show the shape, not the final API.

- **Live widgets.** A column may ask for real components. They are created only for hovered, focused or always-live cells inside the viewport, and are capped. A third-party type's widget is an Isonomic View bound to the cell's value.
- **Editor overlay.** Editing mounts the column type's editor, an Isonomic UI input, select, date picker or checkbox, exactly over the cell. The cell under it is not painted.
- **Chrome** (header cells, menus, filter popovers, tool panels, status bar, context menus, scrollbars) is ordinary Isonomic UI.
- **Styling.** Row and cell styles come from rules (`when predicate, apply style token`), which also drive conditional formatting. Density presets map to the design language's control heights.
- **Change flashes and skeletons** are painter states, honoring reduced motion.

## B9. Interaction

- **Selection.** An active cell plus any number of rectangular ranges; whole-row and whole-column selection; select all. Stored by row ID and column ID, so it survives sorting and filtering. On a windowed source, select-all is stored as "every row matching the query, except these", and a range is stored by its two corner keys; rows that are not loaded are never enumerated. A checkbox column is optional.
- **Bulk actions on rows that are not loaded.** Copy and export stream the selection from the source with a progress indicator and the limits in B19. Edits and fills over such a selection are sent to the source as one predicate-based edit when it declares that capability, and are refused otherwise.
- **Keyboard.** Arrows move; Shift extends; Command-arrow jumps to the edge of data; Tab and Enter move and commit; Page Up / Down, Home / End; typing starts an edit; F2 or Enter edits in place; Escape cancels; Space toggles checkboxes; Delete clears.
- **Editing.** Per-type editors, validation with inline errors, commit and cancel, read-only columns and cells. Every edit is a transaction on an undo stack (edits, pastes, fills, row adds and deletes, reorders).
- **Fill handle.** Drag to copy or continue a series (numbers, dates, patterns).
- **Clipboard.** The framework clipboard carries text with attached metadata, images and file paths; it has no HTML flavor. Copy writes tab-separated text with the typed payload attached as metadata on that entry. Paste reads the typed payload when present, otherwise parses the text; it maps the block onto the selection, parses through each column's type, reports rejects, and is one undo step. An HTML table flavor, for spreadsheets that prefer it, depends on a clipboard request to Isonomic UI shared with the editor.
- **Columns.** Resize, reorder by drag, hide, pin left or right, fit, all from the header and a column menu; a column tool panel lists everything.
- **Rows.** Reorder by drag when the view is unsorted; add and delete; expand groups and detail rows.
- **Filtering UI.** A per-column menu chosen by type (text contains, number ranges, date presets, a checklist of values with counts), a filter builder for nested AND / OR, a quick-filter box, and find-in-grid with next and previous.
- **Status bar.** Count, sum, average, minimum and maximum of the current selection; row totals; filter state.

## B10. Grouping, aggregation, tree data, pivot

- **Grouping.** By one or more columns; group rows are full-width or sit in an auto group column; expansion state is part of the query.
- **Aggregation.** Sum, average, count, distinct count, minimum, maximum, first, last, plus registered functions; shown in group rows, footer rows and a grand total.
- **Tree data.** Rows with a parent reference or a path render as an expandable hierarchy.
- **Master / detail.** A row expands to a full-width detail region that hosts any element, including another grid.
- **Pivot.** Row fields, column fields, value fields and filters; generated column groups; subtotals and grand totals; drill-through from a value to its source rows. The pivot reads the same snapshot as the flat view and never changes the rows.

## B11. View state

`GridViewState` is serializable: column order, widths, visibility and pinning, the `ViewQuery`, density, and optionally selection and scroll anchor. An app saves several per table as named views. State changes emit events so an app can persist them.

## B15. Accessibility and languages

Accessibility in this framework is attached to elements, and a drawn surface has none. So the grid adds a **semantic shadow** built from the UI library's unstyled table parts, which paint nothing:

- a table with the total row and column counts;
- one row element per visible row, with its one-based index and a name made of its cell labels (from each column type's formatter), plus selected and expanded states;
- for the **active row only**, one element per visible cell with its column index and label, and one per visible header cell.

That is about one hundred elements for a full window, small enough to build every frame, so it does not depend on detecting assistive technology. Moving the active cell moves the cell elements. Sort, filter and edit results are announced.

Keyboard operation is complete without a pointer. Right-to-left layout mirrors pinning and alignment. Number, date and currency formats follow the locale. Roles and keyboard start at M1; a full screen-reader pass is an M6 exit criterion.

## B16. Performance budgets

Budgets are for the Intel test Mac, release build, one million rows unless stated. The "measured" column is the scratch benchmark on a 2-core 2.1 GHz Linux machine (A3): single operations in isolation, not the full pipeline.

| Measure | Budget | Measured in isolation |
|---|---|---|
| Frame time while scrolling, about 2,500 visible cells | under 6 ms; 60 fps held | not measurable here; M0 spike |
| Open 1 million rows by 50 columns (memory) to first frame | under 100 ms after load | n/a |
| Sort by one numeric key | under 150 ms, off the main thread | 51 ms |
| Sort by two keys | under 200 ms | 68 ms |
| First sort of a text column by collation (250,000 distinct values) | under 1.5 s, with progress, old order kept | 1,071 ms |
| Later sorts of the same text column | under 150 ms | an integer sort, about 50 ms |
| Filter, numeric predicate | under 50 ms | 5.8 ms |
| Filter, text contains | under 150 ms | 16 to 69 ms |
| Group by one column with two aggregates | under 150 ms | 2.5 to 49 ms |
| Pivot with one row field and one column field | under 250 ms | 2.8 ms |
| Height index update or lookup | under 1 µs | 83 ns |
| Apply 10,000 live updates per second | no dropped frames | n/a |
| Windowed source, 100 million rows | scroll and jump stay exact (rule 7) | n/a |
| Memory, 1 million rows by 20 mixed columns | under 400 MB | n/a |

CI enforces deterministic counters (cells painted per frame, text shapes per frame, shadow elements per frame, pipeline stages re-run per change), not timings. Timings come from `cargo xtask report` and go in `docs/status.md`.
