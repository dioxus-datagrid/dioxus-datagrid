# Release process

## Why a phase ends in a release

`dx components add --git …` installs the component **from the default branch** and builds it against
the crate versions in `component.json` — that is, against crates.io. If the component on `main` uses
crate APIs that are not published yet, a user's installation does not compile.

So: a phase that binds the registry components to new crate APIs is **published before it is
pushed** (ADR-0019). The `dx components add (crates.io)` job in the Registry workflow is the alarm:
while it is red, the documented installation is broken for anyone who tries it.

## The steps

1. **Finish the phase.** All acceptance criteria met, the three commands green, the end-to-end suite
   green, `scripts/registry-smoke.sh` green.
2. **Raise the versions.** `crates/datagrid-core/Cargo.toml`, `crates/dioxus-datagrid/Cargo.toml`,
   the core's dependency on itself, and the `dioxus-datagrid` requirement in all three
   `registry/*/component.json`.
3. **Close the changelog.** `[Unreleased]` becomes `[x.y.z] - YYYY-MM-DD`.
4. **Release commit**, then `cargo publish --dry-run` for both crates.
5. **Publish** — `cargo publish -p datagrid-core`, then `-p dioxus-datagrid` once the first is on
   crates.io. This is the maintainer's own step; nobody else's credentials are involved.
6. **Verify against the published crates**, which is what a user will get:

   ```bash
   SMOKE_PUBLISHED=1 bash scripts/registry-smoke.sh
   ```

7. **Push**, and wait for CI, E2E and Registry — all three jobs — to go green.
8. **Tag** the release only after that, and only deliberately.

Publishing to crates.io and creating or pushing a tag are the two points where the work stops and
the maintainer decides. Both are listed in
[`CLAUDE.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/CLAUDE.md) as things
never done on someone's behalf.

## Version history

| Version | Phase |
|---|---|
| 0.8.0 | Grouping and aggregates: multi-level groups, a group panel, the treegrid |
| 0.7.0 | Editing: cells, rows, a form dialog, batches, validation |
| 0.6.0 | Typed filters: operators, filter menu, value lists |
| 0.5.0 | Foundation: typed cell values, formats, `GridLocale` |
| 0.4.0 | Server-side data: `DataSource`, `GridQuery`, the fullstack example |
| 0.3.0 | Column widths and the column picker |
| 0.2.0 | Virtualization |
| 0.1.0 | Sorting, filtering, search, paging, selection, the ARIA grid |

The details are in
[`CHANGELOG.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/CHANGELOG.md).
