#!/usr/bin/env sh
# Verify the Rust daml-fmt backend against the corpus, using the existing
# three-tier oracle (CLAUDE.md): parse -> desugar-equivalence -> idempotence.
#
#   tools/verify-rust.sh              # idempotence + curated desugar subset
#   tools/verify-rust.sh --no-desugar # idempotence only
#   tools/verify-rust.sh --desugar    # full SDK desugar-equivalence tier
#   FMT=path/to/daml-fmt tools/verify-rust.sh   # use a prebuilt binary
#
# Tiers:
#   idempotence  (always)            format(format(x)) == format(x)
#   equivalence  (default subset)     desugar(format(x)) byte-identical to desugar(x)
#   equivalence  (--desugar)          same check over the whole corpus
#
# Idempotence is the bar the Rust harness owns and is fast. Desugar is the
# semantic oracle; a curated representative subset runs by default so common
# regressions fail close to the formatter change. The full desugar sweep is slow
# (two SDK invocations per file), so it remains explicit.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

desugar_mode=subset
case "${1:-}" in
  "")
    ;;
  "--no-desugar")
    desugar_mode=none
    ;;
  "--desugar" | "--desugar=full")
    desugar_mode=full
    ;;
  "--desugar=subset")
    desugar_mode=subset
    ;;
  *)
    echo "usage: tools/verify-rust.sh [--no-desugar|--desugar|--desugar=subset]" >&2
    exit 2
    ;;
esac

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
desugar_list="$tmp/desugar-list.txt"; : > "$desugar_list"

if [ "$desugar_mode" = full ]; then
  cp "$list" "$desugar_list"
elif [ "$desugar_mode" = subset ]; then
  # Curated standalone files that exercise formatter-sensitive syntax:
  # do/let/case, record updates, choices, interfaces, exceptions, scripts, and
  # template shapes. They desugar without project context, so default verification
  # gives a real semantic signal without paying the full corpus cost.
  for rel in \
    sdk/docs/sharable/sdk/reference/daml/code-snippets/Account.daml \
    sdk/docs/sharable/sdk/reference/daml/code-snippets/Snippets.daml \
    sdk/docs/sharable/sdk/reference/daml/code-snippets/PatternMatching.daml \
    sdk/docs/sharable/sdk/reference/daml/code-snippets-dev/Exceptions.daml \
    sdk/docs/sharable/sdk/reference/daml/code-snippets-dev/Interfaces.daml \
    sdk/compiler/damlc/tests/daml-test-files/ChoiceSyntaxes.daml \
    sdk/compiler/damlc/tests/daml-test-files/ApplicativeDo.daml \
    sdk/compiler/damlc/tests/daml-test-files/RecordUpdate.daml \
    sdk/compiler/damlc/tests/daml-test-files/InterfaceSyntax.daml \
    sdk/compiler/damlc/tests/daml-test-files/ExceptionTryCatch.daml \
    sdk/templates/skeleton/main/daml/Main.daml \
    sdk/templates/skeleton-single-package/daml/Main.daml
  do
    f="$root/original/$rel"
    [ -f "$f" ] || { echo "missing desugar subset file: $f" >&2; exit 2; }
    echo "$f" >> "$desugar_list"
  done
fi

one="$tmp/one.daml"; two="$tmp/two.daml"
while IFS= read -r f; do
  "$FMT" "$f" > "$one"
  "$FMT" "$one" > "$two"
  cmp -s "$one" "$two" || echo "$f" >> "$nonidem"
done < "$list"

checked_desugar=0
if [ "$desugar_mode" != none ] && command -v daml >/dev/null 2>&1; then
  while IFS= read -r f; do
    checked_desugar=$((checked_desugar + 1))
    "$FMT" "$f" > "$one"
    base=$(basename "$f")
    d="$tmp/desugar"; rm -rf "$d"; mkdir -p "$d"; cp "$one" "$d/$base"
    a=$(cd "$(dirname "$f")" && daml --no-legacy-assistant-warning damlc desugar "$base" -o - 2>/dev/null || true)
    b=$(cd "$d" && daml --no-legacy-assistant-warning damlc desugar "$base" -o - 2>/dev/null || true)
    if [ -z "$a" ] || [ -z "$b" ]; then
      echo "$f (desugar failed)" >> "$neq"
    elif [ "$a" != "$b" ]; then
      echo "$f" >> "$neq"
    fi
  done < "$desugar_list"
fi

n_nonidem=$(wc -l < "$nonidem" | tr -d ' ')
n_neq=$(wc -l < "$neq" | tr -d ' ')

echo "files:          $total"
echo "non-idempotent: $n_nonidem"
if [ "$desugar_mode" = none ]; then
  echo "desugar: SKIPPED (--no-desugar)"
elif command -v daml >/dev/null 2>&1; then
  echo "desugar-mode:   $desugar_mode"
  echo "desugar-files:  $checked_desugar"
  echo "desugar-not-equivalent: $n_neq"
else
  echo "desugar: SKIPPED (Daml SDK not on PATH)"
fi
[ "$n_nonidem" -gt 0 ] && { echo "--- non-idempotent ---"; cat "$nonidem"; }
[ "$n_neq" -gt 0 ] && { echo "--- desugar-not-equivalent ---"; cat "$neq"; }
[ "$n_nonidem" -eq 0 ] && [ "$n_neq" -eq 0 ]
