#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

workspace_package_version() {
  local package="$1"
  cargo metadata --no-deps --format-version=1 --locked \
    | jq -r --arg package "${package}" '.packages[] | select(.name == $package) | .version'
}

crates_io_probe_status() {
  local crate="$1"
  local version="$2"
  curl -sS -o /dev/null -w '%{http_code}' \
    -H 'User-Agent: daml-tools-package-check' \
    "https://crates.io/api/v1/crates/${crate}/${version}"
}

package_flags=()
if ! git diff --quiet || ! git diff --cached --quiet; then
  if [[ "${ALLOW_DIRTY_PACKAGE:-}" != "1" ]]; then
    echo "Working tree is dirty; refusing cargo package verification." >&2
    echo "Commit changes or set ALLOW_DIRTY_PACKAGE=1 for an intentional local-only check." >&2
    exit 1
  fi
  echo "Working tree is dirty; continuing with cargo package --allow-dirty (ALLOW_DIRTY_PACKAGE=1)." >&2
  package_flags+=(--allow-dirty)
fi

verify_package() {
  local package="$1"
  echo "Verifying ${package} against crates.io dependencies..."
  cargo package -p "${package}" --locked "${package_flags[@]}"
}

verify_packaged_tests() {
  local package="$1"
  local version="$2"
  local extracted="target/package/${package}-${version}"

  echo "Running packaged tests for ${package} ${version}..."
  cargo package -p "${package}" --locked "${package_flags[@]}" >/dev/null
  rm -rf "${extracted}"
  local tarball
  tarball="$(ls -1t "target/package/${package}-"*.crate | head -1)"
  mkdir -p "${extracted}"
  tar -xf "${tarball}" -C "${extracted}" --strip-components=1
  (
    cd "${extracted}"
    cargo test --all-features --locked
  )
}

check_published_internal_dependency_metadata() {
  local metadata
  local temp_dir
  local packages=(daml-parser daml-syntax daml-lint daml-fmt)

  temp_dir="$(mktemp -d)"
  trap "rm -rf '${temp_dir}'" EXIT

  metadata="${temp_dir}/metadata.json"
  cargo metadata --no-deps --format-version=1 --locked >"${metadata}"

  for package in "${packages[@]}"; do
    local version
    local probe_status
    local current_deps
    local published_deps

    version="$(jq -r --arg package "${package}" '.packages[] | select(.name == $package) | .version' "${metadata}")"
    probe_status="$(crates_io_probe_status "${package}" "${version}")"

    case "${probe_status}" in
      200)
        ;;
      404)
        echo "${package} ${version} is not on crates.io yet; skipping published dependency metadata comparison."
        continue
        ;;
      *)
        handle_unexpected_probe_status "${package}" "${version}" "${probe_status}"
        ;;
    esac

    current_deps="${temp_dir}/${package}.current.tsv"
    published_deps="${temp_dir}/${package}.published.tsv"

    jq -r --arg package "${package}" '
      .packages[]
      | select(.name == $package)
      | .dependencies[]
      | select((.kind == null or .kind == "normal") and (.name | startswith("daml-")))
      | [.name, .req]
      | @tsv
    ' "${metadata}" | sort >"${current_deps}"

    curl -sS -H 'User-Agent: daml-tools-package-check' \
      "https://crates.io/api/v1/crates/${package}/${version}/dependencies" \
      | jq -r '
          .dependencies[]
          | select(.kind == "normal" and (.crate_id | startswith("daml-")))
          | [.crate_id, .req]
          | @tsv
        ' \
      | sort >"${published_deps}"

    if ! diff -u "${published_deps}" "${current_deps}"; then
      echo "Published ${package} ${version} has different internal dependency requirements than this workspace." >&2
      echo "Bump ${package}'s version when a published daml-* dependency requirement changes; crates.io versions are immutable." >&2
      exit 1
    fi
  done
}

handle_unexpected_probe_status() {
  local crate="$1"
  local version="$2"
  local probe_status="$3"

  case "${probe_status}" in
    429)
      echo "crates.io rate limited the ${crate} ${version} probe (HTTP 429)." >&2
      exit 1
      ;;
    5??)
      echo "crates.io server error for ${crate} ${version} probe (HTTP ${probe_status})." >&2
      exit 1
      ;;
    *)
      echo "unexpected crates.io response for ${crate} ${version} probe (HTTP ${probe_status})." >&2
      exit 1
      ;;
  esac
}

echo "Checking published internal dependency metadata for already-published crate versions..."
check_published_internal_dependency_metadata

echo "Verifying daml-parser (no internal registry dependencies)..."
verify_package daml-parser

parser_version="$(workspace_package_version daml-parser)"
parser_probe_status="$(crates_io_probe_status daml-parser "${parser_version}")"

case "${parser_probe_status}" in
  200)
    echo "daml-parser ${parser_version} is on crates.io; verifying daml-syntax..."
    ;;
  404)
    echo "daml-parser ${parser_version} is not on crates.io yet (HTTP 404)."
    echo "Skipping downstream package verification until release-plz publishes it."
    echo "Re-run this script after the parser release lands on crates.io."
    exit 0
    ;;
  *)
    handle_unexpected_probe_status daml-parser "${parser_version}" "${parser_probe_status}"
    ;;
esac

verify_package daml-syntax

syntax_version="$(workspace_package_version daml-syntax)"
syntax_probe_status="$(crates_io_probe_status daml-syntax "${syntax_version}")"

case "${syntax_probe_status}" in
  200)
    echo "daml-syntax ${syntax_version} is on crates.io; verifying tool crates..."
    for package in daml-lint daml-fmt; do
      verify_package "${package}"
    done
    fmt_version="$(workspace_package_version daml-fmt)"
    verify_packaged_tests daml-fmt "${fmt_version}"
    ;;
  404)
    echo "daml-syntax ${syntax_version} is not on crates.io yet (HTTP 404)."
    echo "Skipping daml-lint and daml-fmt package verification until release-plz publishes it."
    echo "Re-run this script after the syntax release lands on crates.io."
    ;;
  *)
    handle_unexpected_probe_status daml-syntax "${syntax_version}" "${syntax_probe_status}"
    ;;
esac
