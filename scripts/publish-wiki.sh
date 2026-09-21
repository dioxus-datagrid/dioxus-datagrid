#!/usr/bin/env bash
# Mirrors wiki/ into the GitHub wiki.
#
# The pages live in this repository so they are reviewed in pull requests and
# versioned with the code they describe; the wiki is only a rendering of them.
# Edits made in the wiki's web editor are therefore lost on the next run — edit
# wiki/ instead.
#
# The wiki repository does not exist until the wiki has one page: create any
# page once through GitHub's web interface, then run this.
#
#   bash scripts/publish-wiki.sh            push if anything changed
#   DRY_RUN=1 bash scripts/publish-wiki.sh  show the diff, push nothing
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="$repo_root/wiki"
wiki_remote="${WIKI_REMOTE:-https://github.com/dioxus-datagrid/dioxus-datagrid.wiki.git}"
dry_run="${DRY_RUN:-0}"

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

if ! git clone --quiet --depth 1 "$wiki_remote" "$work_dir/wiki" 2>/dev/null; then
  echo "Could not clone $wiki_remote." >&2
  echo "A repository has no wiki remote until its first page exists — create one" >&2
  echo "page in the wiki's web interface, then run this again." >&2
  exit 1
fi

# The wiki is a flat set of pages named after the file. wiki/README.md explains
# the folder to a reader of this repository and has no place in the wiki itself.
find "$work_dir/wiki" -maxdepth 1 -name '*.md' -delete
for page in "$source_dir"/*.md; do
  [[ "$(basename "$page")" == "README.md" ]] && continue
  cp "$page" "$work_dir/wiki/"
done

cd "$work_dir/wiki"

if git diff --quiet && [[ -z "$(git status --porcelain)" ]]; then
  echo "The wiki is already up to date."
  exit 0
fi

git add --all
git --no-pager diff --cached --stat

if [[ "$dry_run" != "0" ]]; then
  echo "DRY_RUN — nothing pushed."
  exit 0
fi

git commit --quiet --message "docs: mirror wiki/ from $(git -C "$repo_root" rev-parse --short HEAD)"
git push --quiet origin HEAD
echo "Wiki updated."
