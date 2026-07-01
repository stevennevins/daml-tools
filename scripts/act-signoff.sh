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

# Stagger parallel starts. `signoff:all` fans these scripts out concurrently,
# and act clones every workflow action (jdx/mise-action, actions/checkout, ...)
# from GitHub per job. Cloning all of them at the same instant exhausts macOS
# ephemeral ports ("can't assign requested address"). A short random jitter
# spreads the initial clone burst.
python3 -c 'import random, time; time.sleep(random.uniform(0, 5))'

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

  # Retry transient infrastructure failures. Fanning ~9 jobs out in parallel
  # (signoff:all) saturates the network two ways, both unrelated to the
  # workflow under test:
  #   1. act cloning actions from GitHub on the host (ephemeral-port
  #      exhaustion, dropped connections).
  #   2. rustup/mise downloading toolchains and tools inside each container
  #      (TLS handshake EOF/timeout, request-send errors, download failures).
  # A backed-off retry lets contention subside, so it is safe to retry these.
  if grep -Eqi 'address already in use|assign requested address|unable to clone|connection reset|connection refused|tls handshake (eof|timeout)|i/o timeout|error sending request for url|could not download file|net/http: (request canceled|timeout)' "${log_file}"; then
    cleanup_failed_containers
    if [ "${attempt}" -lt "${max_attempts}" ]; then
      backoff=$(( attempt * 5 + RANDOM % 10 ))
      echo "act transient infra failure detected; retrying in ${backoff}s with fresh ports (attempt $((attempt + 1))/${max_attempts})" >&2
      sleep "${backoff}"
      attempt=$((attempt + 1))
      continue
    fi
  fi

  exit "${status}"
done
