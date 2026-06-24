#!/usr/bin/env bash
set -euo pipefail

workspace_parser_version() {
  cargo metadata --no-deps --format-version=1 \
    | jq -r '.packages[] | select(.name == "daml-parser") | .version'
}

crates_io_probe_status() {
  local crate="$1"
  local version="$2"
  curl -sS -o /dev/null -w '%{http_code}' \
    -H 'User-Agent: daml-tools-package-check' \
    "https://crates.io/api/v1/crates/${crate}/${version}"
}

verify_package() {
  local package="$1"
  echo "Verifying ${package} against crates.io dependencies..."
  cargo package -p "${package}" --allow-dirty --locked
}

echo "Verifying daml-parser (no internal registry dependencies)..."
verify_package daml-parser

parser_version="$(workspace_parser_version)"
probe_status="$(crates_io_probe_status daml-parser "${parser_version}")"

case "${probe_status}" in
  200)
    echo "daml-parser ${parser_version} is on crates.io; verifying downstream crates..."
    for package in daml-syntax daml-lint daml-fmt; do
      verify_package "${package}"
    done
    ;;
  404)
    echo "daml-parser ${parser_version} is not on crates.io yet (HTTP 404)."
    echo "Skipping downstream package verification until release-plz publishes it."
    echo "Re-run this script after the parser release lands on crates.io."
    ;;
  429)
    echo "crates.io rate limited the daml-parser ${parser_version} probe (HTTP 429)." >&2
    exit 1
    ;;
  5??)
    echo "crates.io server error for daml-parser ${parser_version} probe (HTTP ${probe_status})." >&2
    exit 1
    ;;
  *)
    echo "unexpected crates.io response for daml-parser ${parser_version} probe (HTTP ${probe_status})." >&2
    exit 1
    ;;
esac
