#!/usr/bin/env bash
# Registry smoke test.
#
# Proves the distribution path end to end: a project that knows nothing about
# this repo runs `dx components add data_grid` against our registry and still
# compiles afterwards. Run from anywhere; paths resolve against the repo root.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$repo_root/tests/fixtures/consumer-app"
registry="$repo_root/registry"

echo "==> Resetting the consumer fixture"
rm -rf "$fixture/src/components" "$fixture/assets" "$fixture/Cargo.lock"

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
  "assets/dx-datagrid-theme.css"
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

echo "==> Compiling the consumer app"
cargo check --manifest-path "$fixture/Cargo.toml"

echo "==> Registry smoke test passed"
