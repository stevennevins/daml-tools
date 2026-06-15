#!/usr/bin/env sh
# Verify the Rust daml-fmt backend against the corpus, using the existing
# three-tier oracle (CLAUDE.md): parse -> desugar-equivalence -> idempotence.
#
#   tools/verify-rust.sh              # build release, idempotence over original/
#   tools/verify-rust.sh --desugar    # also run the SDK desugar-equivalence tier
#   FMT=path/to/daml-fmt tools/verify-rust.sh   # use a prebuilt binary
#
# Tiers:
#   idempotence  (always)            format(format(x)) == format(x)
#   equivalence  (--desugar + `daml`) desugar(format(x)) byte-identical to desugar(x)
#
# Idempotence is the bar the Rust harness owns and is fast. The desugar tier is
# slow (two SDK invocations per file) so it is opt-in; the manual sweeps in
# CLAUDE.md remain the canonical equivalence check.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

do_desugar=0
[ "${1:-}" = "--desugar" ] && do_desugar=1

FMT=${FMT:-}
if [ -z "$FMT" ]; then
  cargo build --release --bin daml-fmt >&2
  # Workspace builds land in the shared workspace target dir, not $root/target.
  target_dir=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
  FMT="$target_dir/release/daml-fmt"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
list="$tmp/list.txt"
find "$root/original" -name '*.daml' | sort > "$list"
total=$(wc -l < "$list" | tr -d ' ')

nonidem="$tmp/nonidem.txt"; : > "$nonidem"
neq="$tmp/neq.txt"; : > "$neq"

one="$tmp/one.daml"; two="$tmp/two.daml"
while IFS= read -r f; do
  "$FMT" "$f" > "$one"
  "$FMT" "$one" > "$two"
  cmp -s "$one" "$two" || echo "$f" >> "$nonidem"

  [ "$do_desugar" = 1 ] || continue
  command -v daml >/dev/null 2>&1 || continue
  base=$(basename "$f")
  d="$tmp/desugar"; rm -rf "$d"; mkdir -p "$d"; cp "$one" "$d/$base"
  a=$(cd "$(dirname "$f")" && daml --no-legacy-assistant-warning damlc desugar "$base" -o - 2>/dev/null || true)
  b=$(cd "$d" && daml --no-legacy-assistant-warning damlc desugar "$base" -o - 2>/dev/null || true)
  [ -n "$a" ] && [ -n "$b" ] && [ "$a" != "$b" ] && echo "$f" >> "$neq" || true
done < "$list"

n_nonidem=$(wc -l < "$nonidem" | tr -d ' ')
n_neq=$(wc -l < "$neq" | tr -d ' ')

echo "files:          $total"
echo "non-idempotent: $n_nonidem"
if [ "$do_desugar" = 1 ] && command -v daml >/dev/null 2>&1; then
  echo "desugar-not-equivalent: $n_neq"
else
  echo "desugar: SKIPPED (pass --desugar with the SDK on PATH)"
fi
[ "$n_nonidem" -gt 0 ] && { echo "--- non-idempotent ---"; cat "$nonidem"; }
[ "$n_neq" -gt 0 ] && { echo "--- desugar-not-equivalent ---"; cat "$neq"; }
[ "$n_nonidem" -eq 0 ] && [ "$n_neq" -eq 0 ]
