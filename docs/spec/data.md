# Isonomic Grid: Data model, sources and the view query

## B4. Data model

```rust
pub enum Value {
    Null, Bool(bool), Int(i64), Float(f64), Decimal(Decimal), Text(Arc<str>),
    Date(Date), DateTime(DateTime), Duration(Duration),
    Choice(ChoiceId), Choices(SmallVec<ChoiceId>),
    Ref(RowRef), Json(Arc<JsonValue>), Bytes(Arc<[u8]>), Custom(CustomValue),
}
pub struct Column { pub id: ColumnId, pub name: SharedText, pub ty: TypeId, pub config: TypeConfig }
```

- **Column types** are a registry. A type provides: `format`, `parse`, `compare`, the filter operators it supports, the aggregations it supports, a painter, an editor, and a default width and alignment. Built in at M1: text, number, currency, percent, checkbox, date, date-time, single choice, multi choice, link. Later: person, file, relation, rollup, rating, progress, sparkline, computed.
- **Three tiers of author.** Built-in types live here. First-party app crates register native types (Rust painter and editor). A third-party type is declared as data: a base type, a format pattern, options, a validation expression, and optionally an Isonomic View used as the cell's live widget and editor. The view reads the cell's value as host data and writes it by calling one host action; it never paints, and it lives only in live cells (B19 caps how many). This repository does not depend on the views engine; the adapter is registered where both are linked (B18).
- **Storage.** The in-memory store holds one typed vector per column with a validity bitmap. An optional `grid-arrow` adapter crate maps standard columnar record batches without copying; it is a feature, so the default build stays light.
- **Every text column has a dictionary,** built the first time the column is sorted, filtered or grouped: distinct values once, a code per row. Grouping, set filters, search and sorting all run over codes.
- **Collation.** Text sorts by locale-aware collation, with a natural-order option ("item 2" before "item 10"). The collator ranks the dictionary's distinct values once; rows then sort by rank as integers. The rank is cached until the column's data changes.
- **Rows.** `RowId` is a 64-bit ID assigned by the source. Views are index vectors over rows, never copies.
- **Transactions.** `add`, `update`, `remove` in batches, each producing a change set the pipeline and the view consume incrementally.

## B5. Data sources

```rust
pub trait DataSource: Send + Sync + 'static {
    fn schema(&self) -> Arc<Schema>;
    fn capabilities(&self) -> Capabilities;   // which query stages it runs itself
    fn fetch(&self, request: FetchRequest) -> BoxFuture<'static, Result<Page>>;
    fn apply(&self, edits: Vec<Edit>) -> BoxFuture<'static, Result<Applied>>;
    fn changes(&self) -> ChangeStream;
}

pub struct FetchRequest {
    pub query: Arc<ViewQuery>,
    pub group_path: Vec<Value>,   // empty: top level; otherwise the keys of the group being opened
    pub page: PageRequest,        // Range(start..end) | After(RowKey, n) | Before(RowKey, n)
}
pub struct Page { pub rows: Rows, pub count: RowCount }   // Exact(n) | AtLeast(n) | Unknown
```

Version 0.1 implements this contract synchronously, for the in-memory source only: the same methods, returning their results directly. The asynchronous form above arrives with windowed sources.

| Mode | Who runs the query | Use |
|---|---|---|
| **Memory** | The engine, on background threads, over the columnar store. | Up to tens of millions of cells loaded locally; artifacts; pasted and imported data. |
| **Windowed** | The source. The grid sends the `ViewQuery` and asks for pages, group levels and aggregates on demand. | Capsule tables through the SDK's SQL interface; any remote or very large data. |
| **Hybrid** | The source for what it declares in `capabilities`, the engine for the rest over the loaded pages. | Sources that sort and filter but cannot pivot. |

- **Group path.** With grouping on, the top level returns group rows; opening a group fetches its children by its path of keys. Flat data always uses an empty path.
- **Seek paging.** `After` and `Before` ask for rows next to a known row key in the current sort order. SQL sources answer these from an index at any depth; offset ranges remain for jumps and for sources that prefer them.
- **Inexact counts.** A source may not know its total. The scrollbar works with an exact count, a lower bound that grows as pages arrive, or no count at all.
- **Cache.** Pages are cached in blocks with an eviction budget; rows not yet loaded paint a skeleton.
- **Wire limits.** SDK frames are capped at 1 MiB, so pages are sized by bytes as well as rows. The SDK's live queries push the whole new result, so the grid subscribes only to the visible window and re-subscribes when scrolling settles.

## B6. The view query

```rust
pub struct ViewQuery {
    pub columns: Vec<ColumnId>,         // projection and order
    pub filter: Option<FilterExpr>,     // AND / OR tree of typed predicates
    pub search: Option<SearchSpec>,     // quick filter across columns
    pub sort: Vec<SortKey>,             // multi-column
    pub group_by: Vec<ColumnId>,
    pub aggregates: Vec<Aggregate>,     // per column, several allowed
    pub pivot: Option<PivotSpec>,
    pub expanded: Expansion,            // which groups are open
}
```

- It is plain data: serializable, comparable, and safe to hand to an agent or a SQL adapter.
- **Pipeline (memory mode):** filter, then sort, then group, then aggregate, then flatten by expansion. Each stage caches its output and recomputes only what a change set touches. A changed sort does not re-run the filter.
- **Facets:** distinct values with counts and numeric ranges per column, computed against the other columns' filters, to feed filter menus.
- **Functions** (filters, comparators, aggregations) are named entries in registries, so a query never contains a closure.
- **Executor notes (memory mode).** Filters produce bitmaps and combine with bitwise operations. Sorting orders (key, row) pairs, with keys taken from numbers directly and from text through the collated rank. Grouping aggregates over dictionary codes where it can and hashes otherwise. Every stage works on a snapshot, is cancelable by generation, and reports progress when it runs longer than 200 ms; the previous order stays on screen until the new one is ready. Whether to split a sort across threads is decided by measurement on the target machine.
- **One expression language.** Validation rules, computed columns, conditional formatting and third-party type manifests use the same expression language as Isonomic Views, through the `ExprEngine` trait (B13).

## B20. Edits against a source, and schema operations

- **Optimistic edits.** An edit shows at once and is sent to the source. The cell is *pending* until the source answers. A rejection puts the cell in a *failed* state with the source's message and restores the old value on dismissal.
- **Conflicts.** If a live update changes a cell that has a pending local edit, the source's answer decides; the cell flashes and the status bar says a newer value arrived. An open editor is never overwritten: the user sees the change on commit and chooses.
- **Undo** against a source sends the inverse edit; it can fail like any edit and says so.
- **Schema operations** (the Tables app needs them): add, rename, reorder, retype and delete a column; add and delete rows; all through `DataSource`, each with a preview. Retyping shows how many values convert and how many would be lost before it runs.

## B21. Semantics shared by every executor

The memory executor and every data source must agree. A source that cannot honor a rule says so in its capabilities, and the engine does that stage itself.

| Topic | Rule |
|---|---|
| Nulls in sorting | Always last, ascending and descending. Pushed down explicitly, never left to a source's default. |
| Not-a-number | After every number, before nulls. |
| Ties | Broken by row ID, so every sort is total and repeatable. |
| Text comparison | By the view's locale collation. A source that sorts text must use the same named collation, or the engine sorts that column. |
| Text search and "contains" | Case-insensitive and accent-insensitive by default; exact is an option per filter. |
| Empty text and null | Different values. "Is empty" matches both; "is null" matches only null. |
| Numbers | Integers and decimals are exact; floats compare by total order. Sum of decimals is exact. |
| Dates and times | A date has no zone. A date-time is stored in UTC and shown, filtered and grouped in the view's time zone. Relative filters ("this week") use the view's zone and week start. |
| Aggregates | Skip nulls. Count has three forms: rows, values (non-null), distinct. Average of no values is null. |
| Choices | Sort by the option order defined on the column, not by label. |
| Group order | By the grouping column's own sort rule; the null group is last. |

A reference test suite states these as cases. Both executors, and every source adapter, must pass it.
