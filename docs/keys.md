# Keys and pointer

"Command" below is Command on macOS and Control elsewhere.

## Moving and selecting

| Key | Does |
|---|---|
| Arrows | Move the active cell |
| Shift with arrows | Extend the selected range |
| Page Up, Page Down | Move a screen; with Shift, extend |
| Home, End | First or last column of the row; with Shift, extend |
| Command with Left or Right | The same as Home and End |
| Command with Up or Down | First or last row of the column; with Shift, extend |
| Command with Home or End | First or last cell of the grid |
| Tab, Shift with Tab | Next or previous cell, wrapping to the next or previous row |
| Shift with Space | Select the active cell's row |
| Control with Space | Select the active cell's column |
| Command with A | Select everything |

## Editing

| Key | Does |
|---|---|
| Any character | Start an edit that replaces the cell |
| Enter, F2, double click | Open the cell with its own text |
| Enter | Confirm and move down |
| Escape | Cancel the edit |
| Arrows, Home, End | In a cell opened with its own text, move the caret; in an edit begun by typing, confirm and move |
| Backspace, Delete | Remove before or after the caret; with no edit open, Backspace starts an empty edit and Delete clears the selection |
| Command with C, X, V | Copy, cut and paste tab-separated text |
| Command with Z, Shift with Command and Z | Undo and redo |

Text a column cannot hold is refused: the cell stays open, marked, until the text is fixed or the edit is cancelled. A paste with such a field changes nothing.

## Pointer

| Gesture | Does |
|---|---|
| Click | Make a cell active; with Shift extend the range to it; with Command start another range |
| Drag from a cell | Select a range; held past an edge, scroll that way |
| Double click a cell | Open it with its own text |
| Click a group row | Open or close the group |
| Click a column title | Sort by it: ascending, descending, then not at all |
| Drag a column title | Move the column |
| Drag a column's right edge | Resize it |
| Double click a column's right edge | Fit it to the text it shows |
| Wheel, scrollbars | Scroll |
