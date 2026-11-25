# Status

Newest first.

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
2. **Text columns are not dictionary-encoded in storage.** The sort builds a dictionary per query, so grouping by a low-cardinality text column costs 181 ms where the spike measured 2.5 ms on encoded data. Encoding text at rest, with a cached rank per column, is the largest single speed-up available and the spec already calls for it.
3. **Text order is folded, not collated.** Lower case with common Latin accents removed, then bytes. Locale-aware collation goes behind the same dictionary once the collation dependency is approved.
4. **Sorting emits every row.** A windowed result (top of the order only) and running the pipeline off the main thread are milestone 2.
5. **Nothing has run on macOS.** All numbers above are from Linux.
6. Pivot, tree data, computed columns, the columnar interchange adapter, typed clipboard payloads and the semantic shadow for assistive technology are later milestones.
7. Requests to other repositories (the SDK `grid` module reservation and the UI requests in the spec) are not filed.

**Decisions made while building, to confirm.**

- `DataSource` is synchronous in 0.1. The view crate will wrap slow sources in tasks; if the engine source needs an asynchronous trait, this is where it changes.
- Integers, dates, date-times, durations, choices and references share one integer vector; decimals, multi-choice, JSON, bytes and custom values use a general vector until they earn a typed one.
- A comparison between a text cell and a number (or the reverse) matches nothing rather than raising an error.
- Text equality in a filter is exact; contains and starts-with ignore case and accents.
