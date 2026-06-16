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
desugar_warnings="$tmp/desugar-warnings.txt"; : > "$desugar_warnings"
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
    rel=${f#"$root/original/"}
    module=$(sed -n "s/^[[:space:]]*module[[:space:]][[:space:]]*\\([A-Za-z_][A-Za-z0-9_'.]*\\).*/\\1/p" "$f" | head -n 1)
    if [ -n "$module" ]; then
      module_path=$(printf '%s\n' "$module" | tr . /).daml
      case "$rel" in
        "$module_path")
          source_root="$root/original"
          file_arg="$module_path"
          ;;
        *"/$module_path")
          source_prefix=${rel%"$module_path"}
          source_prefix=${source_prefix%/}
          source_root="$root/original/$source_prefix"
          file_arg="$module_path"
          ;;
        *)
          source_root=$(dirname "$f")
          file_arg=$(basename "$f")
          ;;
      esac
    else
      source_root=$(dirname "$f")
      file_arg=$(basename "$f")
    fi

    d="$tmp/desugar"; rm -rf "$d"; mkdir -p "$d/$(dirname "$file_arg")"
    cp "$one" "$d/$file_arg"
    a_file="$tmp/original.desugar"; b_file="$tmp/formatted.desugar"
    rm -f "$a_file" "$b_file"
    a_status=0
    b_status=0
    (cd "$source_root" && daml --no-legacy-assistant-warning damlc desugar "$file_arg" -o "$a_file" 2>/dev/null) || a_status=$?
    (cd "$d" && daml --no-legacy-assistant-warning damlc desugar "$file_arg" -o "$b_file" 2>/dev/null) || b_status=$?
    if [ ! -f "$a_file" ] || [ ! -f "$b_file" ]; then
      echo "$f (desugar produced no output)" >> "$neq"
    elif ! cmp -s "$a_file" "$b_file"; then
      echo "$f" >> "$neq"
    elif [ "$a_status" -ne 0 ] || [ "$b_status" -ne 0 ]; then
      echo "$f (compiler exit $a_status/$b_status; output byte-identical)" >> "$desugar_warnings"
    fi
  done < "$desugar_list"
fi

n_nonidem=$(wc -l < "$nonidem" | tr -d ' ')
n_neq=$(wc -l < "$neq" | tr -d ' ')
n_desugar_warnings=$(wc -l < "$desugar_warnings" | tr -d ' ')

echo "files:          $total"
echo "non-idempotent: $n_nonidem"
if [ "$desugar_mode" = none ]; then
  echo "desugar: SKIPPED (--no-desugar)"
elif command -v daml >/dev/null 2>&1; then
  echo "desugar-mode:   $desugar_mode"
  echo "desugar-files:  $checked_desugar"
  echo "desugar-not-equivalent: $n_neq"
  echo "desugar-compiler-warnings: $n_desugar_warnings"
else
  echo "desugar: SKIPPED (Daml SDK not on PATH)"
fi
[ "$n_nonidem" -gt 0 ] && { echo "--- non-idempotent ---"; cat "$nonidem"; }
[ "$n_neq" -gt 0 ] && { echo "--- desugar-not-equivalent ---"; cat "$neq"; }
[ "$n_desugar_warnings" -gt 0 ] && { echo "--- desugar-compiler-warnings ---"; cat "$desugar_warnings"; }
[ "$n_nonidem" -eq 0 ] && [ "$n_neq" -eq 0 ]
