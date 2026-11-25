# Isonomic Grid: Scope, rules and layout

## B1. Scope

**In scope.** A reusable engine: typed data model, in-memory store, query pipeline, layout math, selection and editing, import and export, and a GPUI grid with its chrome (headers, menus, filter UI, tool panels, status bar).

**Out of scope.** The Tables app (bases, sharing, board / calendar / gallery views). A formula language (that is the expression engine; the grid consumes it through a trait). Charts (Isonomic UI has them; the grid hands them data). SQL itself (data sources live outside this repository).

## B2. Rules

1. **No panic on any input**, including malformed data, pastes and remote changes.
2. **Row IDs are stable.** Position is never identity.
3. **Headless crates stay headless** and build for WebAssembly. `cargo xtask deps` enforces an allow-list.
4. **Only `ui` names the component library; only `grid-arrow` names the columnar interchange library.**
5. **The main thread never sorts, filters, groups or parses.** Heavy work runs on the background executor against a snapshot and is cancelable.
6. **Paint reads, never writes.** Handlers change state, render builds a frame, paint draws it.
7. **The grid owns scrolling.** Position is (anchor row, offset) in 64-bit, never one 32-bit pixel offset, which loses precision beyond a few hundred thousand rows. The grid presents the UI library's scrollbar interface over a scaled range.
8. **Tokens only.** No raw colors, sizes or durations.
9. **Hermetic tests.** No network, no real timers.
10. **Small files.** Every file under 20 KB.
11. **Names.** No other product, repository or account name anywhere except `THIRD_PARTY_NOTICES.md`.
12. **Third-party code never runs natively.** A third-party column type is data plus a view.
13. **Every limit in B19 is enforced** on load, import, paste and live update.
14. **Rows are never compared with the collator.** Text sorts through its dictionary and a cached rank.
15. **One meaning everywhere.** The semantics in B21 hold for the memory executor and for every source; a parity test suite enforces it.

## B3. Repository layout

| Crate | Role | Depends on |
|---|---|---|
| `grid-core` | Values, column types, schema, row IDs, the `DataSource` trait, transactions, the in-memory columnar store. | nothing here |
| `grid-query` | `ViewQuery`, filter expressions, the pipeline (filter, sort, group, aggregate, pivot, flatten), facets, search, incremental maintenance. | core |
| `grid-layout` | Axis geometry, pinned regions, viewport math, hit testing, spans. | core |
| `grid-interact` | Selection model, keyboard navigation, edit sessions, undo, clipboard encode and decode, fill. | core, layout |
| `grid-formats` | CSV, TSV, JSON; XLSX behind a feature. | core |
| `grid-gpui` | `GridState` entity, `Grid` element, painters, editor overlay, headers, menus, filter UI, tool panels, drag interactions. | all above, ui |
| `ui` | Re-export of Isonomic UI. | isonomic-ui |
| `grid-playground` | Demo app with generated datasets up to 10 million rows. | grid-gpui |
| `xtask` | deps check, screenshots, performance report. | |

`default-members` are the headless crates and `xtask`. `corpus/` holds datasets and golden results.
