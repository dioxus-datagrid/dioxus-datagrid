# dioxus-datagrid

A typed, accessible data grid for **Dioxus 0.7**. The logic lives in two crates you depend on; the
styled shell is copied into your project by `dx components add` and is yours from then on.

It runs on web, desktop and mobile, follows the WAI-ARIA grid and treegrid patterns, and works
against rows in memory or against a server.

## Where the project stands

| | |
|---|---|
| Latest release | **0.7.0** — editing (crates.io, docs.rs) |
| On `main`, unreleased | **Phase 10** — grouping and aggregates, due as 0.8.0 |
| Next | Phase 11 — column layout: order, pinning, multi-level headers, column menu |
| Long game | [Roadmap to 1.0](Roadmap.md) — parity with the Syncfusion Blazor DataGrid on the features people actually use |

Every phase ends in a release, so you can stop on any version and have something whole.

## Start here

- **Using it** → [README](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/README.md)
  for installation and a first grid, then the component's own
  [`docs.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/registry/data_grid/docs.md)
  for every prop and column option.
- **What it can do today** → [Feature status](Feature-Status.md), area by area, with the gaps named.
- **How it is built** → [Architecture](Architecture.md): the four layers, the boundaries between
  them, and what happens between a keystroke and a rendered row.
- **Working on it** → [Contributing](Contributing.md) and [Release process](Release-Process.md).

## The shape of it in one screen

```rust
#[derive(Clone, PartialEq)]
struct User { id: u32, name: String, salary: f64 }

impl GridRow for User {
    type Key = u32;
    fn key(&self) -> u32 { self.id }
}

let columns = use_hook(|| vec![
    Column::new("name", "Name")
        .value_text(|u: &User| u.name.as_str())
        .editable(|u: &mut User, name: String| u.name = name),
    Column::new("salary", "Salary")
        .value_of(|u: &User| u.salary)
        .format(CellFormat::currency("€", 2))
        .aggregate(Aggregate::Sum),
]);

rsx! {
    DataGrid { data: users, columns, page_size: 25, selection: SelectionMode::Multi,
        DataGridGroupPanel::<User> {}
        DataGridEditor { mode: EditMode::Row, on_save: save }
    }
}
```

Columns are described once. Sorting, filtering, formatting, grouping, aggregates, editing and export
all read the same typed value, so a column never has to be described twice.

## The map of the documentation

The wiki is the orientation layer. Everything normative lives in the repository and is versioned
with the code:

| Document | What it is | Language |
|---|---|---|
| [README](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/README.md) | Install, first grid, headless use, server-side data | English |
| [`registry/data_grid/docs.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/registry/data_grid/docs.md) | Every prop and column option of the installed component | English |
| [`registry/data_grid_editor/docs.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/registry/data_grid_editor/docs.md) | Editing modes, callbacks, validation | English |
| [`registry/data_grid_group_panel/docs.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/registry/data_grid_group_panel/docs.md) | Grouping, aggregates, grouping on a server | English |
| [`CHANGELOG.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/CHANGELOG.md) | What changed in each version | English |
| [`docs/ACCESSIBILITY.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/ACCESSIBILITY.md) | Exactly which roles, attributes and keys are rendered, and what is checked automatically | English |
| [`docs/DECISIONS.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/DECISIONS.md) | 28 architecture decision records — why things are the way they are | German |
| [`docs/VERIFICATION.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/VERIFICATION.md) | Dioxus APIs checked against the real source before use, with the results | German |
| [`PLAN.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/PLAN.md) / [`docs/ROADMAP.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/ROADMAP.md) | Phases 0–6, then 7–15 and 1.0 | German |
| API reference | [docs.rs/dioxus-datagrid](https://docs.rs/dioxus-datagrid), [docs.rs/datagrid-core](https://docs.rs/datagrid-core) | English |

The planning and decision records are in German because that is the language they were written and
argued in. The public surface — README, changelog, doc comments, component docs — is English.

## Examples

Four, all runnable from the repository:

| | |
|---|---|
| [`examples/basic`](https://github.com/dioxus-datagrid/dioxus-datagrid/tree/main/examples/basic) | The smallest useful grid |
| [`examples/virtualized`](https://github.com/dioxus-datagrid/dioxus-datagrid/tree/main/examples/virtualized) | 100,000 rows on the primitives |
| [`examples/server`](https://github.com/dioxus-datagrid/dioxus-datagrid/tree/main/examples/server) | A simulated server with adjustable latency, grouping in memory |
| [`examples/fullstack`](https://github.com/dioxus-datagrid/dioxus-datagrid/tree/main/examples/fullstack) | SQLite behind a Dioxus server function: a `GridQuery` becomes SQL, including `GROUP BY` |

The [playground](https://github.com/dioxus-datagrid/dioxus-datagrid/tree/main/playground) renders the
registry component with a switch for every mode, and is what the end-to-end tests drive.
