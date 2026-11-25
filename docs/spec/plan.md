# Isonomic Grid: Milestones and risks

## B22. Milestones

| | Scope | Exit criteria |
|---|---|---|
| **M0 Foundations** | Workspace, CI, rules, xtask (deps, pins). Core types and layout math. Reserve the SDK `grid` module. **Paint spike:** one element drawing 1 million by 50 with pinned regions, its own scroll state behind the standard scrollbar driven to 100 million rows, and the semantic shadow, measured. | On the Intel Mac: frame under 6 ms at 2,500 visible cells with the shadow on; more than 95% of text shapes served from the cache while scrolling; shadow under 150 elements. Otherwise the fallback is chosen and recorded. |
| **M1 Read-only grid** | Column types, formatting, resize / reorder / hide / pin / fit, grouped headers, multi-sort through dictionaries and cached collation ranks, selection with ranges, keyboard, copy, CSV export, theming, density, limits, the semantics suite. | A 1 million-row dataset is browsed, sorted and copied within budget. |
| **M2 Filter and find** | Per-type filters, filter builder, quick filter, find, facets, status bar. | Filter menus show live counts; pipeline matches the reference implementation. |
| **M3 Editing** | Editors, validation, undo, paste, fill, row add / delete / reorder, live updates with change flashes, pending and failed states. | A sheet can be filled in with the keyboard alone; undo covers everything. |
| **M4 Analysis** | Grouping, aggregation, totals, tree data, master / detail, pivot. | Pivot of 1 million rows inside budget, with drill-through. |
| **M5 Sources and SDK** | Windowed and hybrid sources with group paths and seek paging, block cache, schema operations, saved view state, fill the SDK module, third-party column types through views, editor block, Views component, agent tools. | A Capsule table of 10 million rows scrolls, filters and groups through the SDK. |
| **M6 Advanced** | Computed columns on the expression engine, sparklines, cell notes, spans, automatic row heights, XLSX, full accessibility, charts from a range. | Screen-reader pass on macOS; XLSX round-trip fixtures green. |

## B23. Risks and open decisions

| Risk | Mitigation |
|---|---|
| Painted cells miss the frame budget on the Intel Mac | M0 is exactly this test, before anything is built on it. Fallback: retained per-row elements on the optional faster backend. |
| The semantic shadow is not enough for screen-reader users | It carries row names and the active row's cells; the M6 pass tests real navigation, and the shadow can widen to more rows if needed. |
| Collation differs between the memory executor and SQL | Named collation pushed down; where a source cannot match it, the engine sorts that column; the semantics suite catches drift. |
| First collated sort of a huge text column is slow | Measured at about 1.1 s for a million rows; shown with progress, old order kept, rank cached. |
| The scaled scrollbar feels wrong at extreme sizes | The spike drives 100 million rows through it; a jump-to-row field in the status bar covers precise navigation. |
| Scope: the full feature set is large | The headless split lets three workers run in parallel after M0; milestones ship usable grids from M1. |
| Push-down semantics differ between memory and SQL | One `ViewQuery`, one reference test suite both executors must pass, including collation. |
| Third-party column types through views are too limited | They cover formatting, validation and a widget; first-party apps keep native painters. |
| Build time on the test Mac | Headless crates are default members; the columnar adapter and XLSX are features. |

**For Omar to decide.**

1. Confirm the repository name `isonomic-grid` (approved earlier) for what this request called Isonomic data tables.
2. Approve painted cells as the default, with the M0 gate.
3. Confirm third-party column types are views only, following the Desktop decision. Publish the type manifest specification under Apache-2.0 as an interface, with the implementation GPL?
4. Should pivot (M4) come before Capsule sources (M5), or after? The Tables app needs M5 first; analysis-heavy artifacts need M4 first.
