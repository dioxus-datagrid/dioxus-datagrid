# data_grid_context_menu

A context menu for [`data_grid`](../data_grid/docs.md): a right click, or `Shift+F10` or the menu key,
on a cell or on a column header.

## Usage

```rust
use crate::components::data_grid::DataGrid;
use crate::components::data_grid_context_menu::DataGridContextMenu;

rsx! {
    DataGrid { data: people, columns,
        DataGridContextMenu::<Person> {}
    }
}
```

## What it offers

- **On a column header:** what the column menu offers: sort, group, pin, hide, fit the width.
- **On a cell:** Copy (the value as the grid shows it), Copy row (tab-separated), and, where the grid can do
  them: Select or Deselect row, Show or Hide details, Edit, Delete. Delete acts on the selection if the row
  is in it, and on the row alone otherwise.

An entry that would do nothing is left out, and a target with nothing to offer leaves the right click to
the browser. A cell that is being edited keeps the browser's own menu.

## Your own entries

```rust
DataGridContextMenu::<Person> {
    entries: move |target: ContextTarget| match target {
        ContextTarget::Cell { row_index, .. } => vec![
            ContextEntry::new("mail", "Write an email", Callback::new(move |_| write_to(row_index))),
        ],
        _ => Vec::new(),
    },
}
```

They come after the grid's, behind a line. `ContextEntry::disabled` shows one that cannot be chosen now.

## Keyboard

`Shift+F10` or the menu key opens the menu at the focused cell. Inside it, `ArrowDown` and `ArrowUp` move
between entries and wrap, `Home` and `End` jump to the ends, `Enter` or `Space` chooses, and `Escape`,
`Tab` or a click outside close it and put the focus back on the cell. It is a `menu` of `menuitem`s, and
`menuitemradio` where the entries are a choice.

It opens at the pointer, or at the cell for the keyboard, and is moved back inside the window if it would
stick out, on either axis and in either direction of text.
