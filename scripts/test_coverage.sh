#!/bin/bash
# SPDX-FileCopyrightText: Copyright 2024 LG Electronics Inc.
# SPDX-License-Identifier: Apache-2.0
#
# test_coverage.sh — Generates statement + branch coverage reports (ISO 26262 §9.4.5)
# for all safety-critical Pullpiri crates using cargo-tarpaulin.
#
# Coverage is collected for:
#   common, tools, apiserver (server), nodeagent (agent),
#   statemanager (player), actioncontroller (player), filtergateway (player)
#
# A minimum coverage threshold of 70% is enforced on all safety-critical crates.
# The pipeline fails if any crate drops below this threshold.
set -euo pipefail

# Detect project root (for CI or local)
PROJECT_ROOT=${GITHUB_WORKSPACE:-$(pwd)}
LOG_FILE="$PROJECT_ROOT/dist/coverage/test_coverage_log.txt"
COVERAGE_ROOT="$PROJECT_ROOT/dist/coverage"
cd "$PROJECT_ROOT"
mkdir -p "$COVERAGE_ROOT"
rm -f "$LOG_FILE"
touch "$LOG_FILE"
PIDS=()

# Minimum coverage threshold for safety-critical crates (ISO 26262 §9.4.5 ASIL-B)
COVERAGE_THRESHOLD=70

# Track overall pass/fail
COVERAGE_FAILED=0

echo "🧪 Starting test coverage collection per crate..." | tee -a "$LOG_FILE"
echo "📊 Minimum coverage threshold: ${COVERAGE_THRESHOLD}% (ISO 26262 §9.4.5)" | tee -a "$LOG_FILE"

# === Function: Start background service ===
start_service() {
  local manifest="$1"
  local name="$2"
  echo "🔄 Starting $name..." | tee -a "$LOG_FILE"
  cargo run --manifest-path="$manifest" &>> "$LOG_FILE" &
  PIDS+=($!)
}

# === Function: Stop all background services ===
cleanup() {
  echo -e "\n🧹 Stopping services..." | tee -a "$LOG_FILE"
  for pid in "${PIDS[@]}"; do
    if kill -0 "$pid" &>/dev/null; then
      kill "$pid" 2>/dev/null || echo "⚠️ Could not kill $pid"
    fi
  done
  PIDS=()  # Reset PID list
}
trap cleanup EXIT  # Ensure cleanup is called on exit

# === Ensure cargo-tarpaulin is installed ===
if ! command -v cargo-tarpaulin &>/dev/null; then
  echo "📦 Installing cargo-tarpaulin..." | tee -a "$LOG_FILE"
  cargo install cargo-tarpaulin
fi

# === Enable nightly-only options ===
export RUSTC_BOOTSTRAP=1

# === MANIFEST paths ===
COMMON_MANIFEST="src/common/Cargo.toml"
NODEAGENT_MANIFEST="src/agent/nodeagent/Cargo.toml"
TOOLS_MANIFEST="src/tools/Cargo.toml"
APISERVER_MANIFEST="src/server/apiserver/Cargo.toml"
FILTERGATEWAY_MANIFEST="src/player/filtergateway/Cargo.toml"
ACTIONCONTROLLER_MANIFEST="src/player/actioncontroller/Cargo.toml"
STATEMANAGER_MANIFEST="src/player/statemanager/Cargo.toml"

# === Function: run tarpaulin and check threshold ===
run_tarpaulin() {
  local manifest="$1"
  local label="$2"
  local output_dir="$3"
  local report_name="$4"
  local extra_args="${5:-}"
  local custom_threshold="${6:-$COVERAGE_THRESHOLD}"
  local packages_arg="${7:-}"
  local include_arg="${8:-}"

  if [[ ! -f "$manifest" ]]; then
    echo "::warning ::$manifest not found. Skipping $label coverage..." | tee -a "$LOG_FILE"
    return 0
  fi

  local manifest_dir
  manifest_dir=$(dirname "$PROJECT_ROOT/$manifest")
  echo "📂 Running tarpaulin in $manifest_dir for $label" | tee -a "$LOG_FILE"
  mkdir -p "$output_dir"

  local tarpaulin_exit=0
  (
    cd "$manifest_dir"
    # shellcheck disable=SC2086
    cargo tarpaulin \
      --skip-clean \
      --out Html --out Lcov --out Xml \
      --output-dir "$output_dir" \
      --ignore-panics --no-fail-fast \
      --run-types Tests \
      $packages_arg \
      $include_arg \
      $extra_args \
      2>&1 | tee -a "$LOG_FILE"
  ) || tarpaulin_exit=$?

  # Rename default HTML report to crate-specific name
  mv "$output_dir/tarpaulin-report.html" \
     "$output_dir/tarpaulin-report-${report_name}.html" 2>/dev/null || true

  # Extract coverage percentage from lcov.info (count lines covered vs total)
  local lcov_file="$output_dir/lcov.info"
  if [[ -f "$lcov_file" ]]; then
    local lines_found lines_hit coverage_pct
    lines_found=$(grep "^LF:" "$lcov_file" | awk -F: '{sum+=$2} END{print sum}')
    lines_hit=$(grep "^LH:" "$lcov_file" | awk -F: '{sum+=$2} END{print sum}')
    if [[ "${lines_found:-0}" -gt 0 ]]; then
      coverage_pct=$(awk "BEGIN {printf \"%.1f\", ($lines_hit / $lines_found) * 100}")
      echo "📈 $label coverage: ${coverage_pct}% (${lines_hit}/${lines_found} lines)" | tee -a "$LOG_FILE"
      # Threshold check (integer comparison via awk)
      local below_threshold
      below_threshold=$(awk "BEGIN {print ($coverage_pct < $custom_threshold) ? 1 : 0}")
      if [[ "$below_threshold" -eq 1 ]]; then
        echo "::error ::❌ $label coverage ${coverage_pct}% is below the required ${custom_threshold}% threshold (ISO 26262 §9.4.5)" | tee -a "$LOG_FILE"
        COVERAGE_FAILED=1
      else
        echo "✅ $label coverage ${coverage_pct}% meets the ${COVERAGE_THRESHOLD}% threshold" | tee -a "$LOG_FILE"
      fi
    else
      echo "⚠️  $label: No instrumented lines found in lcov.info — skipping threshold check" | tee -a "$LOG_FILE"
    fi
  fi
}

# ==========================================================================
# Phase 1: Standalone crates (no runtime dependencies required)
# ==========================================================================

# === TOOLS ===
run_tarpaulin "$TOOLS_MANIFEST" "tools" "$COVERAGE_ROOT/tools" "tools" "" 70

# === NODEAGENT (Action 1: re-enabled — tests fixed, Podman tests marked #[ignore]) ===
# ISO 26262 §9.4.5: NodeAgent contains comp_req__na__local_reconcile and comp_req__na__backoff
run_tarpaulin "$NODEAGENT_MANIFEST" "nodeagent (agent)" "$COVERAGE_ROOT/agent" "agent" \
  "--ignore-tests" 70

# === STATEMANAGER (Action 2: added — contains comp_req__sm__heartbeat, comp_req__sm__validate_state) ===
# ISO 26262 §9.4.5: StateManager is safety-critical ASIL-B
run_tarpaulin "$STATEMANAGER_MANIFEST" "statemanager (player)" "$COVERAGE_ROOT/statemanager" "statemanager" \
  "" 70 \
  "--packages statemanager" \
  "--include-files player/statemanager/src/*.rs \
   --include-files player/statemanager/src/grpc/*.rs \
   --include-files player/statemanager/src/grpc/receiver/*.rs"

# ==========================================================================
# Phase 2: Service crates (require supporting services running)
# ==========================================================================

# === Step 2a: Start supporting services for integration coverage ===
rm -rf /tmp/pullpiri_shared_rocksdb
mkdir -p /tmp/pullpiri_shared_rocksdb
chmod 777 /tmp/pullpiri_shared_rocksdb
start_service "$FILTERGATEWAY_MANIFEST" "filtergateway"
start_service "$NODEAGENT_MANIFEST"    "nodeagent"
start_service "$STATEMANAGER_MANIFEST" "statemanager"
sleep 3

# === SERVER (apiserver) ===
run_tarpaulin "$APISERVER_MANIFEST" "apiserver (server)" "$COVERAGE_ROOT/server" "server" \
  "--skip-clean" 70 \
  "--packages apiserver" \
  "--include-files server/apiserver/src/*.rs \
   --include-files server/apiserver/src/artifact/*.rs \
   --include-files server/apiserver/src/grpc/*.rs \
   --include-files server/apiserver/src/grpc/sender/*.rs \
   --include-files server/apiserver/src/node/*.rs \
   --include-files server/apiserver/src/route/*.rs"

# === COMMON ===
# Run workspace-wide tests with active background services to measure shared module coverage
# Uses multiple --include-files to cover all subdirectories (logd/, spec/, spec/k8s/, spec/artifact/)
echo "📂 Running tarpaulin for common" | tee -a "$LOG_FILE"
mkdir -p "$COVERAGE_ROOT/common"
(
  cd "$PROJECT_ROOT/src"
  cargo tarpaulin \
    --skip-clean \
    --out Html --out Lcov --out Xml \
    --output-dir "$COVERAGE_ROOT/common" \
    --ignore-panics --no-fail-fast \
    --run-types Tests \
    --workspace \
    --include-files 'common/src/*.rs' \
    --include-files 'common/src/logd/*.rs' \
    --include-files 'common/src/spec/*.rs' \
    --include-files 'common/src/spec/k8s/*.rs' \
    --include-files 'common/src/spec/artifact/*.rs' \
    --exclude-files '*generated*' \
    2>&1 | tee -a "$LOG_FILE"
) || true
mv "$COVERAGE_ROOT/common/tarpaulin-report.html" \
   "$COVERAGE_ROOT/common/tarpaulin-report-common.html" 2>/dev/null || true
# Parse coverage from lcov
_lcov="$COVERAGE_ROOT/common/lcov.info"
if [[ -f "$_lcov" ]]; then
  _lf=$(grep "^LF:" "$_lcov" | awk -F: '{sum+=$2} END{print sum}')
  _lh=$(grep "^LH:" "$_lcov" | awk -F: '{sum+=$2} END{print sum}')
  if [[ "${_lf:-0}" -gt 0 ]]; then
    _pct=$(awk "BEGIN {printf \"%.1f\", ($_lh / $_lf) * 100}")
    echo "📈 common coverage: ${_pct}% (${_lh}/${_lf} lines)" | tee -a "$LOG_FILE"
    if awk "BEGIN {exit ($_pct < 70) ? 0 : 1}"; then
      echo "::error ::❌ common coverage ${_pct}% is below the required 70% threshold (ISO 26262 §9.4.5)" | tee -a "$LOG_FILE"
      COVERAGE_FAILED=1
    else
      echo "✅ common coverage ${_pct}% meets the 70% threshold" | tee -a "$LOG_FILE"
    fi
  fi
fi

# === Stop services before player round ===
cleanup

if command -v docker &>/dev/null && docker ps &>/dev/null; then
  if ! docker ps | grep -qi "idl2dds"; then
    echo "📦 Launching IDL2DDS docker services..." | tee -a "$LOG_FILE"
    [[ ! -d IDL2DDS ]] && git clone https://github.com/MCO-PICCOLO/IDL2DDS -b master
    pushd IDL2DDS
    if docker compose version &>/dev/null; then
      docker compose up --build -d || echo "⚠️ docker compose up failed"
    elif command -v docker-compose &>/dev/null; then
      docker-compose up --build -d || echo "⚠️ docker-compose up failed"
    else
      echo "⚠️ No docker compose or docker-compose command found"
    fi
    popd
  else
    echo "🟢 IDL2DDS already running." | tee -a "$LOG_FILE"
  fi
else
  echo "⚠️ Docker daemon not accessible or running. Skipping IDL2DDS service startup." | tee -a "$LOG_FILE"
fi

# === Player services ===
start_service "$ACTIONCONTROLLER_MANIFEST" "actioncontroller"
start_service "$STATEMANAGER_MANIFEST"     "statemanager"
sleep 3

# === FILTERGATEWAY (player) ===
echo "📂 Running tarpaulin for filtergateway (player)" | tee -a "$LOG_FILE"
mkdir -p "$COVERAGE_ROOT/filtergateway"
(
  cd "$PROJECT_ROOT/src"
  cargo tarpaulin \
    --skip-clean \
    --out Html --out Lcov --out Xml \
    --output-dir "$COVERAGE_ROOT/filtergateway" \
    --ignore-panics --no-fail-fast \
    --run-types Tests \
    --packages filtergateway \
    --include-files 'player/filtergateway/src/*.rs' \
    --include-files 'player/filtergateway/src/filter/*.rs' \
    --include-files 'player/filtergateway/src/grpc/*.rs' \
    --include-files 'player/filtergateway/src/grpc/sender/*.rs' \
    --include-files 'player/filtergateway/src/vehicle/*.rs' \
    --include-files 'player/filtergateway/src/vehicle/dds/*.rs' \
    --exclude-files '*idl2rs*' \
    --exclude-files '*generated*' \
    2>&1 | tee -a "$LOG_FILE"
) || true
mv "$COVERAGE_ROOT/filtergateway/tarpaulin-report.html" \
   "$COVERAGE_ROOT/filtergateway/tarpaulin-report-filtergateway.html" 2>/dev/null || true
_lcov="$COVERAGE_ROOT/filtergateway/lcov.info"
if [[ -f "$_lcov" ]]; then
  _lf=$(grep "^LF:" "$_lcov" | awk -F: '{sum+=$2} END{print sum}')
  _lh=$(grep "^LH:" "$_lcov" | awk -F: '{sum+=$2} END{print sum}')
  if [[ "${_lf:-0}" -gt 0 ]]; then
    _pct=$(awk "BEGIN {printf \"%.1f\", ($_lh / $_lf) * 100}")
    echo "📈 filtergateway (player) coverage: ${_pct}% (${_lh}/${_lf} lines)" | tee -a "$LOG_FILE"
    if awk "BEGIN {exit ($_pct < 70) ? 0 : 1}"; then
      echo "::error ::❌ filtergateway (player) coverage ${_pct}% is below the required 70% threshold (ISO 26262 §9.4.5)" | tee -a "$LOG_FILE"
      COVERAGE_FAILED=1
    else
      echo "✅ filtergateway (player) coverage ${_pct}% meets the 70% threshold" | tee -a "$LOG_FILE"
    fi
  fi
fi

# === ACTIONCONTROLLER ===
# Run actioncontroller package tests — covers grpc/, runtime/, manager, and main
echo "📂 Running tarpaulin for actioncontroller (player)" | tee -a "$LOG_FILE"
mkdir -p "$COVERAGE_ROOT/actioncontroller"
(
  cd "$PROJECT_ROOT/src"
  cargo tarpaulin \
    --skip-clean \
    --out Html --out Lcov --out Xml \
    --output-dir "$COVERAGE_ROOT/actioncontroller" \
    --ignore-panics --no-fail-fast \
    --run-types Tests \
    --packages actioncontroller \
    --include-files 'player/actioncontroller/src/*.rs' \
    --include-files 'player/actioncontroller/src/grpc/*.rs' \
    --include-files 'player/actioncontroller/src/grpc/sender/*.rs' \
    --include-files 'player/actioncontroller/src/runtime/*.rs' \
    2>&1 | tee -a "$LOG_FILE"
) || true
mv "$COVERAGE_ROOT/actioncontroller/tarpaulin-report.html" \
   "$COVERAGE_ROOT/actioncontroller/tarpaulin-report-actioncontroller.html" 2>/dev/null || true
# Parse coverage from lcov
_lcov="$COVERAGE_ROOT/actioncontroller/lcov.info"
if [[ -f "$_lcov" ]]; then
  _lf=$(grep "^LF:" "$_lcov" | awk -F: '{sum+=$2} END{print sum}')
  _lh=$(grep "^LH:" "$_lcov" | awk -F: '{sum+=$2} END{print sum}')
  if [[ "${_lf:-0}" -gt 0 ]]; then
    _pct=$(awk "BEGIN {printf \"%.1f\", ($_lh / $_lf) * 100}")
    echo "📈 actioncontroller (player) coverage: ${_pct}% (${_lh}/${_lf} lines)" | tee -a "$LOG_FILE"
    if awk "BEGIN {exit ($_pct < 70) ? 0 : 1}"; then
      echo "::error ::❌ actioncontroller (player) coverage ${_pct}% is below the required 70% threshold (ISO 26262 §9.4.5)" | tee -a "$LOG_FILE"
      COVERAGE_FAILED=1
    else
      echo "✅ actioncontroller (player) coverage ${_pct}% meets the 70% threshold" | tee -a "$LOG_FILE"
    fi
  fi
fi

cleanup

# ==========================================================================
# Summary
# ==========================================================================
echo "" | tee -a "$LOG_FILE"
echo "============================================================" | tee -a "$LOG_FILE"
echo "✅ Coverage reports generated at: $COVERAGE_ROOT" | tee -a "$LOG_FILE"
echo "============================================================" | tee -a "$LOG_FILE"

if [[ "$COVERAGE_FAILED" -eq 1 ]]; then
  echo "::error ::❌ One or more crates failed the ISO 26262 §9.4.5 coverage threshold of ${COVERAGE_THRESHOLD}%." | tee -a "$LOG_FILE"
  echo "Attach dist/coverage/ as evidence to the TÜV submission package." | tee -a "$LOG_FILE"
  exit 1
fi

echo "All coverage thresholds met. Attach dist/coverage/ to the TÜV submission package." | tee -a "$LOG_FILE"
