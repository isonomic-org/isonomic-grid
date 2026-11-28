# Requests to other repositories

What Isonomic Grid needs from repositories it does not own. Each is written to be filed as it stands. **None has been filed yet.**

## To Isonomic UI

### G1. An HTML flavor on the clipboard, read and write

**Need.** Copy a range as an HTML table as well as tab-separated text, and read an HTML table on paste. Spreadsheets prefer the HTML flavor and lose types and line breaks without it.

**Today.** The clipboard carries text with attached metadata, images and file paths. The grid writes tab-separated text and parses the same on paste.

**Shared with** Isonomic Editor's request R5.

### G2. Measuring text without drawing it

**Need.** A column sized to fit its contents, from a double click on its edge. That needs the width of a string in the cell font without painting it.

**Today.** The grid can shape a line through the window's text system during paint. A public, documented way to do the same from an event handler, or a `measure` on the text seam, would remove the need to wait for a frame.

### G3. The scrollbar's hit area over a custom viewport

**Need.** The standard scrollbars sit over the grid, which keeps its own scroll state and paints its own cells. They work. Two things would make it cleaner: a documented way to give the scrollbar a viewport that excludes a header band (the grid positions an absolutely placed box for this today), and a way to learn the track width the theme gives, which is currently `#[doc(hidden)]`.

## To the SDK

### G4. Reserve a `grid` module in the app crate

**Need.** A feature-gated, empty `grid` module in the SDK's app crate, as the `editor` module already is, to hold the data source that reads a table through the SDK: windowed results, seek paging, group paths and live updates for the visible window only.

**Why now.** It fixes the name and the feature flag before anything depends on them. The module stays empty until the grid's windowed sources are built.

**Note.** The SDK and this engine are both GPL-3.0-or-later since 28 Nov 2026, so the licence no longer decides this. To keep the SDK free of engine dependencies, the adapter should still live where both are already linked, not in the SDK.
