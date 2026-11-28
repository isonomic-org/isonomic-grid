# Isonomic Grid: Formats, computed columns, agents, testing and integration

## B12. Formats

CSV and TSV import with type inference and a preview; CSV, TSV and JSON export of the current view or selection, with or without headers and formatting. XLSX import and export behind a feature at M6. Print and PDF are left to the host.

## B13. Computed columns

A computed column holds an expression and a result type. The grid calls an `ExprEngine` trait to compile and evaluate; Isonomic's expression engine implements it. Until that exists, computed columns accept registered Rust functions. Dependencies between columns are tracked so an edit recomputes only affected cells.

## B14. Agents

The headless crates are the agent's grid. Tools through the harness at M5:

- `table.describe`: schema, row count, facets.
- `table.query`: run a `ViewQuery`, return rows or aggregates with a row limit.
- `table.edit`: a batch of typed edits addressed by row ID and column ID.
- `table.view`: read or set the user's current view state, so an agent can show what it found.

Every call passes the identity library's check function. Agent edits join the undo stack under the agent's identity.

## B17. Testing

- **Model tests.** The pipeline is checked against a naive reference implementation on random data and random queries.
- **Property tests.** Layout round-trips (offset to index to offset); selection survives any sort and filter; edit followed by undo restores the data; paste of copied data is the identity.
- **Golden files** for CSV round-trips and formatted output across locales.
- **Headless view tests** for keyboard and editing with the UI framework's test context.
- **Screenshots** of the playground scenes, light and dark.
- **Fuzz targets** for CSV, clipboard and filter-expression parsers.

## B18. Integration

- **Isonomic UI.** Reached only through `ui`. Its simple table stays as is.

| From Isonomic UI | Use |
|---|---|
| Inputs, select, date picker, checkbox, popovers, menus | Editors, filter UI, column and context menus |
| Scrollbar and its handle interface | Standard scrollbars over the grid's own scroll state |
| Unstyled semantic table parts | The semantic shadow (B15) |
| Grouped undo history | The edit undo stack |
| Edge auto-scroll, charts | Drag selection; charts from a range |
| **Not reused:** the virtual list and the existing data table's state | They need every size up front, work in 32-bit pixels and build an element per cell |

- **Requests** (text measuring for fit-to-content, overlay positioning over a custom viewport, an HTML clipboard flavor) are filed in `isonomic-ui/docs/requests.md`.
- **Dependency directions.** The engines never depend on one another: Isonomic Views, Isonomic Editor and Isonomic Grid each depend on Isonomic UI only. Adapters that join two engines (view-backed cell widgets, the grid block in a document, a grid component for views) live where both are already linked.
- **Pin parity.** This repository's Isonomic UI pin must equal the SDK's. `cargo xtask pins` compares them and CI fails on a difference.
- **SDK.** The SDK app crate reserves modules for the editor and the node graph, not for the grid. At M0 an empty `grid` module is reserved behind a `grid` feature, so filling it at M5 breaks no app. The SDK and the engines are all GPL-3.0-or-later since 28 Nov 2026, so the licence no longer decides this; the recommendation is still to keep adapters in the window repository or a GPL crate of their own. These are shared-contract changes and take the `needs-master` label. A SQL data source adapter over the SDK's `sql` interface lives with the Tables app, not here.
- **Isonomic Editor.** A grid block embeds a grid in a document.
- **Isonomic Views.** A grid component gives artifacts a real table over inline or bound data. Views also render third-party column widgets.
- **Charts.** A selected range can be handed to Isonomic UI's charts.
- **Convenience layer.** `Grid::from_rows` for a quick in-memory grid, and a derive that turns a Rust row type into a schema, for first-party apps.

## B19. Limits

Data can come from other people and other programs. Each limit is enforced on load, import, paste and live update. Exceeding one degrades (truncate, placeholder, or refuse) and never panics.

| Limit | Default |
|---|---|
| Columns | 2,000 |
| Text painted per cell | 2,000 characters; the rest is reachable in the editor and by copy |
| Cell value size | 1 MiB |
| Import file size | 2 GiB, streamed |
| Paste size | 64 MiB |
| Live native widgets alive at once | 200 |
| Live view-backed widgets alive at once | 50 |
| Rows copied or exported from a source in one action | 1 million, streamed, cancelable |
| Distinct values shown in a set filter | 10,000, then search only |
| Filter expression depth | 16 |
