#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-semver-checks >/dev/null; then
  echo "cargo-semver-checks is required: cargo install cargo-semver-checks --locked" >&2
  exit 1
fi

failed=()
for package in daml-parser daml-syntax daml-lint daml-fmt; do
  if ! cargo semver-checks check-release --package "${package}"; then
    failed+=("${package}")
  fi
done

if ((${#failed[@]} > 0)); then
  printf 'cargo-semver-checks reports API breaks in:' >&2
  printf ' %s' "${failed[@]}" >&2
  printf '\n' >&2
  exit 1
fi
