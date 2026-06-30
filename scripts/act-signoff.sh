#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 <workflow-file> <job> [act args...]" >&2
  exit 2
fi

workflow="$1"
job="$2"
shift 2

slug="${workflow##*/}-${job}"
slug="${slug//[^A-Za-z0-9_.-]/-}"
run_id="$(date +%Y%m%d%H%M%S)-$$-${RANDOM}"
run_root=".act/signoff-runs/${slug}/${run_id}"
mkdir -p "${run_root}"

free_port() {
  python3 - <<'PY'
import socket
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.bind(("", 0))
    print(s.getsockname()[1])
PY
}

cleanup_failed_containers() {
  # act names containers from workflow/job names, not from our run_root. Do not
  # remove successful reusable containers globally; only ask act to remove failed
  # workflow containers through --rm below.
  true
}

max_attempts=5
attempt=1
while [ "${attempt}" -le "${max_attempts}" ]; do
  cache_port="$(free_port)"
  artifact_port="$(free_port)"
  attempt_root="${run_root}/attempt-${attempt}"
  mkdir -p "${attempt_root}"
  log_file="${attempt_root}/act.log"

  echo "act signoff run: workflow=${workflow} job=${job} attempt=${attempt} cache_port=${cache_port} artifact_port=${artifact_port}" >&2

  set +e
  act workflow_dispatch \
    -W "${workflow}" \
    -j "${job}" \
    --rm \
    --action-cache-path "${attempt_root}/action-cache" \
    --cache-server-path "${attempt_root}/cache-server" \
    --cache-server-port "${cache_port}" \
    --artifact-server-path "${attempt_root}/artifacts" \
    --artifact-server-port "${artifact_port}" \
    "$@" 2>&1 | tee "${log_file}"
  status="${PIPESTATUS[0]}"
  set -e

  if [ "${status}" -eq 0 ]; then
    exit 0
  fi

  if grep -Eq 'bind: address already in use|listen tcp .* address already in use' "${log_file}"; then
    cleanup_failed_containers
    if [ "${attempt}" -lt "${max_attempts}" ]; then
      echo "act port collision detected; retrying with fresh ports" >&2
      attempt=$((attempt + 1))
      continue
    fi
  fi

  exit "${status}"
done
