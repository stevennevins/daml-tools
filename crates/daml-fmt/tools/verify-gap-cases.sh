#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
crate_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_root=$(CDPATH= cd -- "$crate_dir/../.." && pwd)
case_root="$crate_dir/corpus/gap-cases"
out_dir="$repo_root/target/daml-fmt-gap-cases/desugar"

rm -rf "$out_dir"
mkdir -p "$out_dir/bad" "$out_dir/good"

for side in bad good; do
  for file in "$case_root/$side"/*.daml; do
    name=${file##*/}
    base=${name%.daml}
    (
      cd "$case_root/$side"
      daml --no-legacy-assistant-warning damlc desugar "$name" -o -
    ) > "$out_dir/$side/$base.desugar"
  done
done

count=0
for bad in "$out_dir/bad"/*.desugar; do
  name=${bad##*/}
  if [ "$name" = "ImportOrganization.desugar" ]; then
    sed '/^import /d' "$bad" > "$out_dir/bad/$name.no-imports"
    sed '/^import /d' "$out_dir/good/$name" > "$out_dir/good/$name.no-imports"
    left="$out_dir/bad/$name.no-imports"
    right="$out_dir/good/$name.no-imports"
  else
    left="$bad"
    right="$out_dir/good/$name"
  fi
  if ! cmp -s "$left" "$right"; then
    printf 'desugar differs: %s\n' "$name" >&2
    exit 1
  fi
  count=$((count + 1))
done

printf 'compiled and desugar-equivalent: %s pairs\n' "$count"
