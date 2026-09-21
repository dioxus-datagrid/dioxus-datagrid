# The wiki, kept in the repository

These pages are the source of truth for the GitHub wiki at
[github.com/dioxus-datagrid/dioxus-datagrid/wiki](https://github.com/dioxus-datagrid/dioxus-datagrid/wiki).
They live here, and not only in the wiki's own repository, so that they are reviewed in pull
requests, versioned with the code they describe, and found by a search of this repository.

`scripts/publish-wiki.sh` mirrors this folder into the wiki. Edit the files here; do not edit pages
in the wiki's web editor, or the next mirror overwrites them.

## What belongs here, and what does not

The wiki is the **orientation layer**: what this project is, what it can do today, how the pieces
fit together, how to work on it. It answers "where do I start" and "why is it built like that".

Everything normative stays in the repository proper and is only linked from here:

| | |
|---|---|
| How to use it | `README.md` and each component's `docs.md` |
| What changed | `CHANGELOG.md` |
| The API | doc comments, published as docs.rs |
| What is rendered for accessibility | `docs/ACCESSIBILITY.md` |
| Why a thing is the way it is | `docs/DECISIONS.md` |
| What was checked against the real Dioxus | `docs/VERIFICATION.md` |
| What is planned | `PLAN.md`, `docs/ROADMAP.md` |

The rule of thumb: if it describes a specific version, it belongs with that version in the
repository. If it describes the project, it can live here.

## Publishing

The wiki has to be created once through GitHub's web interface — a repository has no wiki git
remote until its first page exists. Create any page there, then:

```bash
bash scripts/publish-wiki.sh
```

The script clones the wiki repository, copies these files over, and pushes if anything changed.
`DRY_RUN=1` shows what it would do without pushing.

Link between these pages with the file name — `[Architecture](Architecture.md)` — so the links work
here as well. The script strips the suffix on its way into the wiki, where a `.md` link serves the
raw file instead of the rendered page. Links into the code repository are absolute and stay as they
are.
