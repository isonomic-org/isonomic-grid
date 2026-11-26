# Progress

| Milestone | State |
|---|---|
| M0 Foundations | Workspace, rules, checks and CI are in. The tests pass and frames render on macOS. The paint spike's frame budget is not met on the Intel Mac at the stress size (6.5 to 8.5 ms at 1,950 cells against 6 ms at 2,500); since then the grid shares shaped lines instead of copying them, paints a band's text in one layer and keeps a drawn cell's line, which halves the frame on the Linux machine (3.7 to 3.9 ms scrolling at 2,496 cells) on either backend. On the Mac scrolling now takes 5.4 to 5.8 ms a frame on the default backend and 5.2 ms on the faster one, inside the 6 ms budget; a scrollbar drag that shows a new screen every frame takes 7 ms on the default backend. The semantic shadow the budget also names is not built. The view builds and passes its tests on the optional faster backend too; rows kept as retained elements are not built and on these numbers not needed. The cross-repository requests are written in `docs/requests.md` and not filed. |
| M1 Core grid | Headless half done: data model, in-memory source, filter, search, sort, layout, selection, navigation, edits with undo, delimited formats. View half started: drawing, selection, keyboard, pointer, in-cell editing, range drag, the clipboard, scrollbars, and columns that resize, move, hide, pin and fit work in headless windows and render on Linux and macOS; not yet used by hand. |
| M2 Scale | Not started. Bitmap filters, dictionary-encoded text and a stable radix sort are in; windowed results and background execution are not. |
| M3 Grouping and aggregation | Headless grouping, aggregates and expansion are in. Tree data and pivot are not. |
| M4 Editing and formats | Typed parsing, paste, cut, fill, range clear, import and a one-line cell editor with composition are in. A menu on a secondary click sorts, pins, fits, hides and shows columns and cuts, copies and pastes. A search bar narrows the rows to those holding the text typed. Pickers and the typed clipboard payload are not. |
| M5 Integration | Not started. |
| M6 Hardening | Not started. |
