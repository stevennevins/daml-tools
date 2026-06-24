#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lint_pkg="crates/daml-lint"

# Paths matched by `npm run check:rules` git diff --exit-code gate.
diff_paths=(
  rules
  examples
  lint-plugin
  tools
  package.json
  package-lock.json
  tsconfig.json
)

staged_paths=()

restore_staging() {
  if ((${#staged_paths[@]} > 0)); then
    git -C "${repo_root}" restore --staged "${staged_paths[@]}"
  fi
}
trap restore_staging EXIT

for rel in "${diff_paths[@]}"; do
  path="${lint_pkg}/${rel}"
  if git -C "${repo_root}" diff --quiet -- "${path}" \
    && git -C "${repo_root}" diff --cached --quiet -- "${path}"; then
    continue
  fi
  git -C "${repo_root}" add -- "${path}"
  staged_paths+=("${path}")
done

(
  cd "${repo_root}/${lint_pkg}"
  npm ci
  npm run check:rules
)
