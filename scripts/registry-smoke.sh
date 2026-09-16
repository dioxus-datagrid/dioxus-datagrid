# ...and leave it that way afterwards, pass or fail, so a local run does not
# leave a tracked file modified.
trap 'git -C "$repo_root" checkout -- "tests/fixtures/consumer-app/Cargo.toml" 2>/dev/null || true' EXIT
#!/usr/bin/env bash
# Registry smoke test.
#
# Proves the distribution path end to end: a project that knows nothing about
# this repo runs `dx components add data_grid` against our registry and still
# compiles afterwards. Run from anywhere; paths resolve against the repo root.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$repo_root/tests/fixtures/consumer-app"
# The repository root, not registry/: `dx components add --git` clones the repo
# and reads component.json at its top level, so that is the entry point users hit.
registry="$repo_root"

echo "==> Resetting the consumer fixture"
rm -rf "$fixture/src/components" "$fixture/assets" "$fixture/Cargo.lock"
# `dx components add` and the patch section below both edit Cargo.toml, so the
# run has to start from the committed version.
git -C "$repo_root" checkout -- "tests/fixtures/consumer-app/Cargo.toml" 2>/dev/null || true
# ...and leave it that way afterwards, pass or fail, so a local run does not
# leave a tracked file modified.
trap 'git -C "$repo_root" checkout -- tests/fixtures/consumer-app/Cargo.toml 2>/dev/null || true' EXIT

echo "==> Listing components in $registry"
cd "$fixture"
dx components list --path "$registry"

echo "==> Adding data_grid"
dx components add data_grid --path "$registry"

echo "==> Verifying the expected files were produced"
expected=(
  "src/components/mod.rs"
  "src/components/data_grid/mod.rs"
  "src/components/data_grid/component.rs"
  "src/components/data_grid/style.css"
  "assets/dx-components-theme.css"
)
for file in "${expected[@]}"; do
  if [[ ! -f "$fixture/$file" ]]; then
    echo "FAIL: expected $file to exist after 'dx components add'" >&2
    exit 1
  fi
done

# Files listed under "exclude" in the manifest must not leak into user projects.
for file in "src/components/data_grid/component.json" "src/components/data_grid/docs.md"; do
  if [[ -f "$fixture/$file" ]]; then
    echo "FAIL: $file is excluded in the manifest but was copied anyway" >&2
    exit 1
  fi
done

if ! grep -q "pub mod data_grid;" "$fixture/src/components/mod.rs"; then
  echo "FAIL: data_grid was not registered in src/components/mod.rs" >&2
  exit 1
fi

# The manifest's cargoDependencies must have landed, or the copied component
# would not compile in a real project.
if ! grep -q "dioxus-datagrid" "$fixture/Cargo.toml"; then
  echo "FAIL: dioxus-datagrid was not added to the fixture's Cargo.toml" >&2
  exit 1
fi

# Point the crates.io dependency at this checkout. Without it the fixture would
# build against the last published release, so the smoke test would pass or fail
# for reasons unrelated to the working tree.
echo "==> Redirecting the crates.io dependency to this checkout"
cat >> "$fixture/Cargo.toml" <<EOF

[patch.crates-io]
# Relative, because Git Bash spells absolute paths as /c/..., which Cargo on
# Windows cannot read. Relative paths in [patch] resolve against this file.
dioxus-datagrid = { path = "../../../crates/dioxus-datagrid" }
datagrid-core = { path = "../../../crates/datagrid-core" }
EOF

echo "==> Compiling the consumer app"
cargo check --manifest-path "$fixture/Cargo.toml"

echo "==> Registry smoke test passed"
