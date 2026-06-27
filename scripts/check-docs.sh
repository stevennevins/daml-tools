#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

fail() {
  echo "$1" >&2
  exit 1
}

crate_version() {
  local package="$1"
  cargo metadata --no-deps --format-version=1 --locked \
    | jq -r --arg package "${package}" '.packages[] | select(.name == $package) | .version'
}

assert_doc_contains() {
  local file="$1"
  local needle="$2"
  local label="$3"
  [[ -f "${file}" ]] || fail "missing doc file: ${file}"
  grep -Fq "${needle}" "${file}" || fail "${label}: expected ${file} to contain '${needle}'"
}

assert_doc_not_contains() {
  local file="$1"
  local needle="$2"
  local label="$3"
  [[ -f "${file}" ]] || fail "missing doc file: ${file}"
  if grep -Fq "${needle}" "${file}"; then
    fail "${label}: ${file} still contains stale '${needle}'"
  fi
}

assert_package_json_version() {
  local file="$1"
  local expected="$2"
  local label="$3"
  [[ -f "${file}" ]] || fail "missing package metadata: ${file}"
  local actual
  actual="$(jq -r '.version' "${file}")"
  [[ "${actual}" == "${expected}" ]] || fail "${label}: expected ${file} version ${expected}, got ${actual}"
}

parser_version="$(crate_version daml-parser)"
syntax_version="$(crate_version daml-syntax)"
lint_version="$(crate_version daml-lint)"
fmt_version="$(crate_version daml-fmt)"

crates_doc="docs/reference/crates.md"
assert_doc_contains "${crates_doc}" "| [\`daml-parser\`](../../crates/daml-parser) | \`${parser_version}\` |" "crate versions"
assert_doc_contains "${crates_doc}" "| [\`daml-syntax\`](../../crates/daml-syntax) | \`${syntax_version}\` |" "crate versions"
assert_doc_contains "${crates_doc}" "| [\`daml-lint\`](../../crates/daml-lint) | \`${lint_version}\` |" "crate versions"
assert_doc_contains "${crates_doc}" "| [\`daml-fmt\`](../../crates/daml-fmt) | \`${fmt_version}\` |" "crate versions"

assert_doc_contains "crates/daml-parser/README.md" "daml-parser = \"${parser_version}\"" "parser README dependency snippet"
assert_doc_contains "crates/daml-lint/README.md" "daml-lint = \"${lint_version}\"" "lint README dependency snippet"
assert_doc_contains "crates/daml-lint/README.md" "daml-lint = { version = \"${lint_version}\", default-features = false }" "lint README no-default-features snippet"
assert_doc_contains "crates/daml-lint/README.md" "current 0.9.x line" "lint README API stability line"
assert_doc_not_contains "crates/daml-lint/README.md" "current 0.8 line" "stale lint README API stability line"
assert_package_json_version "crates/daml-lint/package.json" "${lint_version}" "daml-lint private package.json"
assert_package_json_version "crates/daml-lint/lint-plugin/package.json" "${lint_version}" "lint-plugin package.json"
assert_package_json_version "crates/daml-fmt/package.json" "${fmt_version}" "daml-fmt private package.json"

for stale in "0.8.0" "0.9.0" "0.7.0" "0.2.6"; do
  case "${stale}" in
    0.8.0)
      assert_doc_not_contains "${crates_doc}" "daml-syntax\`](../../crates/daml-syntax) | \`0.8.0\` |" "stale syntax version"
      assert_doc_not_contains "crates/daml-parser/README.md" "daml-parser = \"0.8.0\"" "stale parser README version"
      assert_doc_not_contains "crates/daml-lint/README.md" "daml-lint = \"0.8\"" "stale lint README version"
      assert_doc_not_contains "crates/daml-lint/README.md" "version = \"0.8\"" "stale lint README no-default-features version"
      ;;
    0.9.0)
      assert_doc_not_contains "${crates_doc}" "daml-lint\`](../../crates/daml-lint) | \`0.9.0\` |" "stale lint version"
      ;;
    0.7.0)
      assert_doc_not_contains "${crates_doc}" "daml-fmt\`](../../crates/daml-fmt) | \`0.7.0\` |" "stale fmt version"
      ;;
    0.2.6)
      assert_doc_not_contains "crates/daml-fmt/package.json" "\"version\": \"0.2.6\"" "stale daml-fmt package.json"
      assert_doc_not_contains "docs/how-to/release.md" "0.2.6-rc.0" "stale prerelease example"
      ;;
  esac
done

while IFS= read -r link; do
  [[ -n "${link}" ]] || continue
  case "${link}" in
    http://*|https://*|mailto:*|\#*) continue ;;
  esac
  local_path="${link%%#*}"
  [[ "${local_path}" == ../* || "${local_path}" == ./* ]] || continue
  target="${repo_root}/docs/reference/${local_path}"
  [[ -e "${target}" ]] || fail "broken docs link in ${crates_doc}: ${link} -> ${target}"
done < <(grep -oE '\[[^]]+\]\([^)]+\)' "${crates_doc}" | sed -n 's/.*(\([^)]*\)).*/\1/p')

while IFS= read -r rel; do
  [[ -n "${rel}" ]] || continue
  target="${repo_root}/${rel}"
  [[ -e "${target}" ]] || fail "broken docs link in README.md: ${rel}"
done < <(grep -oE '\[[^]]+\]\(([^)]+)\)' README.md | sed -n 's/.*(\([^)]*\)).*/\1/p' | grep -E '^(docs/|crates/|CONTRIBUTING\.md|LICENSE)')

echo "docs/version checks passed"
