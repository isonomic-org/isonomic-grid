# Isonomic Grid: working rules

Read `docs/spec.md` before changing behavior, and `docs/status.md` for where things stand.

## Rules

1. **No panic on any input.** Every table, file, paste and edit yields a valid state or an error value.
2. **Row IDs are stable and never reused.** Selection, edits and expansion are held by row ID, never by position.
3. **Headless crates stay headless.** `grid-core`, `grid-query`, `grid-layout`, `grid-interact` and `grid-formats` name no UI framework and touch no network, file or process. `cargo xtask deps` enforces the allow-list.
4. **Only `ui` names the component library.**
5. **The view query is plain data.** Anything a view asks of its data fits in `ViewQuery`; nothing reaches around it.
6. **One set of semantics for every executor** (`docs/spec/data.md`): nulls sort last in both directions; not-a-number sorts after numbers and before nulls; ties break by row ID; search ignores case and accents; date-times are UTC; aggregates skip nulls.
7. **Rows are never compared by collating strings.** Text goes through a dictionary and is compared by rank.
8. **Edits against a source are all or nothing,** and undo applies inverse edits through the same source.
9. **Layout does arithmetic, not allocation.** A uniform axis holds no per-row state. Offsets are 64-bit.
10. **Hermetic tests.** No network, no real timers, no randomness without a fixed seed.
11. **Small files.** Every file under 20 KB. `cargo xtask sizes` checks.
12. **Tokens only** in view code: no raw colors, sizes or durations.
13. **Names.** No other product, repository or account name anywhere in this repository except dependency manifests and `THIRD_PARTY_NOTICES.md`.
14. **The engines do not depend on each other.** Adapters between the grid and the editor, views or canvas live where both are already linked.

## Workflow

- One task per branch and pull request. Keep pull requests small enough to review.
- Before pushing: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo xtask deps`, `cargo xtask sizes`.
- A change in behavior comes with a test. A change that touches the pipeline comes with numbers from `cargo run --release -p grid-query --example bench`.
- Update `docs/status.md` (newest first) and `docs/PROGRESS.md` in the same pull request.
- When code and specification disagree, fix one of them in the same pull request and say which.
- Changes to shared contracts (the SDK's wire and app-crate modules, the session event schema, the organization's shared configuration) are requests to those repositories, not edits made from here.
