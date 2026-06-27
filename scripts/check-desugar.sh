#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verify="${repo_root}/crates/daml-fmt/tools/verify-rust.sh"

if ! command -v daml >/dev/null 2>&1; then
  echo "SKIPPED: Daml SDK not on PATH; full desugar verification is optional outside formatter releases." >&2
  echo "Re-run ${verify} after installing Daml SDK 3.4.11 for the compiler desugar oracle." >&2
  exit 0
fi

exec "${verify}" "$@"
