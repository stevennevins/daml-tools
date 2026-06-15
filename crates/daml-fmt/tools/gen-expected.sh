#!/usr/bin/env sh
# Regenerate expected/ from the SHIPPED Rust formatter (replaces the retired
# Node tools/gen-expected.js, which drove the deleted lab/ backend).
#
# expected/ is a snapshot of `daml-fmt`'s own output over original/ — the
# regression baseline npm test compares against. Run after any deliberate
# formatter change, then review the diff and commit.
#
#   tools/gen-expected.sh
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

cargo build --release --bin daml-fmt >&2
# In a Cargo workspace the binary lands in the shared workspace target dir, not
# $root/target. Ask cargo where it is so this works standalone or in-workspace.
target_dir=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
FMT="$target_dir/release/daml-fmt"

n=0
find original -name '*.daml' | sort | while IFS= read -r f; do
  rel="${f#original/}"
  out="expected/$rel"
  mkdir -p "$(dirname "$out")"
  "$FMT" "$f" > "$out"
  n=$((n + 1))
done

echo "regenerated expected/ from $FMT" >&2
