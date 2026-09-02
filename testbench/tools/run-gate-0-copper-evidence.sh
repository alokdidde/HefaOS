#!/usr/bin/env bash
# Retain the frozen Gate 0 raw execution evidence.  The bridge invocation is a
# separate upstream characterization only; it never joins the HefaOS graph.
set -u -o pipefail

readonly copper_revision="fc2ebc4fe3583d1f433b75898ad7c9e4dd9e6af2"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly repository_root="$(cd "${script_dir}/../.." && pwd)"
readonly evidence_root="${HEFAOS_GATE0_EVIDENCE_DIR:-${repository_root}/target/gate-0-copper-evidence/$(date -u +%Y%m%dT%H%M%SZ)}"
readonly commands_dir="${evidence_root}/commands"
readonly upstream_dir="${evidence_root}/upstream/copper-rs"
readonly hefaos_runs="${evidence_root}/hefaos-runs"
readonly nominal_timing_runs="${evidence_root}/nominal-timing"

mkdir -p "${commands_dir}" "${evidence_root}/environment" "${evidence_root}/generated-config"
failed=0

capture() {
    local name="$1"
    local command="$2"
    local command_path="${commands_dir}/${name}.command"
    local status_path="${commands_dir}/${name}.status"
    local status_temporary="${status_path}.tmp.$$"
    if ! rm -f "${status_path}"; then
        printf '%s\n' "error: cannot clear stale status for ${name}" >&2
        return 1
    fi
    if ! printf '%s\n' "${command}" >"${command_path}"; then
        printf '%s\n' "error: cannot write command record for ${name}" >&2
        return 1
    fi
    bash -lc "cd \"${repository_root}\" && ${command}" \
        >"${commands_dir}/${name}.stdout" 2>"${commands_dir}/${name}.stderr"
    local command_status=$?
    if ! printf '%s\n' "${command_status}" >"${status_temporary}" \
        || ! mv -f "${status_temporary}" "${status_path}"; then
        rm -f "${status_temporary}" "${status_path}"
        printf '%s\n' "error: cannot write status record for ${name}" >&2
        return 1
    fi
    return "${command_status}"
}

required() {
    capture "$@" || failed=1
}

required rustc_version "rustup run 1.95.0 rustc -Vv"
required host "uname -a; test -r /etc/os-release && cat /etc/os-release; lscpu"
required revision "git rev-parse HEAD; git status --short"
required source_snapshot "git diff --binary; git diff --cached --binary; git ls-files --others --exclude-standard -z | xargs -0r sha256sum"
required source_clean "git diff --quiet && git diff --cached --quiet && test -z \"\$(git ls-files --others --exclude-standard)\""
required locked_copper "grep -A2 '^name = \"cu29\"$' Cargo.lock"
required generated_config "install -D \"${repository_root}/testbench/copper-spike/copperconfig.ron\" \"${evidence_root}/generated-config/copperconfig.ron\" && sha256sum \"${evidence_root}/generated-config/copperconfig.ron\" > \"${evidence_root}/generated-config/SHA256SUMS\""
if [[ "${failed}" -ne 0 ]]; then
    if ! {
        printf 'copper_revision_required=%s\n' "${copper_revision}"
        printf '%s\n' 'hefaos_spike=not-accepted: environment capture or clean-checkout requirement failed'
        printf '%s\n' 'iceoryx2_control_admission=rejected: no bridge qualification was attempted'
    } >"${evidence_root}/verdict.txt"; then
        printf '%s\n' "error: cannot write evidence verdict" >&2
    fi
    exit 1
fi

# Frozen raw upstream reference: source is retained in this bundle, including
# its commit and command output rather than only an interpreted summary.
required upstream_clone "git clone --depth 1 --branch v1.1.1 https://github.com/copper-project/copper-rs.git \"${upstream_dir}\""
required upstream_revision "git -C \"${upstream_dir}\" rev-parse HEAD; git -C \"${upstream_dir}\" status --short"
required upstream_run_in_sim "cd \"${upstream_dir}\" && rustup run 1.95.0 cargo run -p cu-run-in-sim"

required hefaos_build "/usr/bin/time -v rustup run 1.95.0 cargo build --locked -p hefaos-copper-spike"
required hefaos_fmt "rustup run 1.95.0 cargo fmt --all --check"
required hefaos_clippy_workspace "rustup run 1.95.0 cargo clippy --workspace --all-targets --locked -- -D warnings"
required hefaos_test_workspace "rustup run 1.95.0 cargo test --workspace --all-targets --locked"
required hefaos_nominal_timing "HEFAOS_COPPER_EVIDENCE_DIR=\"${nominal_timing_runs}\" /usr/bin/time -v rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence timing-nominal"
required nominal_timing_log_digests "find \"${nominal_timing_runs}\" -type f -print0 | sort -z | xargs -0r sha256sum"
required nominal_timing_live_log_size "find \"${nominal_timing_runs}\" -type f -name 'live_*.copper' -printf '%s\\n' | awk '{total += \$1} END {print total + 0}'"
required hefaos_run_all "HEFAOS_COPPER_EVIDENCE_DIR=\"${hefaos_runs}\" rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence run-all"
required hefaos_replay_all "HEFAOS_COPPER_EVIDENCE_DIR=\"${hefaos_runs}\" /usr/bin/time -v rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence replay-all"
required spike_binary "stat --format='%n %s bytes' target/debug/hefaos-copper-spike"
required hefaos_log_digests "find \"${hefaos_runs}\" -type f -print0 | sort -z | xargs -0r sha256sum"

# The raw Copper demo is intentionally infinite. A bounded two-process run is
# recorded as an observation (both timeout exit statuses must be 124), not as
# an IPC qualification. The source excerpt preserves the relevant bincode/Vec
# copy and the absent queue/schema/epoch/pool declarations.
required iceoryx2_build "cd \"${upstream_dir}\" && rustup run 1.95.0 cargo build -p cu-iceoryx2-bridge-demo --bins"
required iceoryx2_implementation "sed -n '20,80p' \"${upstream_dir}/components/bridges/cu_iceoryx2_bridge/src/lib.rs\"; sed -n '280,390p' \"${upstream_dir}/components/bridges/cu_iceoryx2_bridge/src/lib.rs\""
required iceoryx2_loopback "( cd \"${upstream_dir}\" && timeout 4s rustup run 1.95.0 cargo run -p cu-iceoryx2-bridge-demo --bin iceoryx2-pong ) > \"${evidence_root}/iceoryx2-pong.stdout\" 2> \"${evidence_root}/iceoryx2-pong.stderr\" & pong=\$!; sleep 1; ( cd \"${upstream_dir}\" && timeout 2s rustup run 1.95.0 cargo run -p cu-iceoryx2-bridge-demo --bin iceoryx2-ping ) > \"${evidence_root}/iceoryx2-ping.stdout\" 2> \"${evidence_root}/iceoryx2-ping.stderr\"; ping_status=\$?; wait \$pong; pong_status=\$?; printf 'ping_timeout_status=%s\\npong_timeout_status=%s\\n' \"\$ping_status\" \"\$pong_status\"; test \"\$ping_status\" -eq 124; test \"\$pong_status\" -eq 124; grep -E 'got pong seq=' \"${evidence_root}/iceoryx2-ping.stdout\""

if ! {
    printf 'copper_revision_required=%s\n' "${copper_revision}"
    printf 'upstream_revision=%s\n' "$(head -n1 "${commands_dir}/upstream_revision.stdout" 2>/dev/null || true)"
    printf 'hefaos_nominal_timing_status=%s\n' "$(cat "${commands_dir}/hefaos_nominal_timing.status" 2>/dev/null || true)"
    printf 'hefaos_run_all_status=%s\n' "$(cat "${commands_dir}/hefaos_run_all.status" 2>/dev/null || true)"
    printf 'hefaos_replay_all_status=%s\n' "$(cat "${commands_dir}/hefaos_replay_all.status" 2>/dev/null || true)"
    printf 'hefaos_fmt_status=%s\n' "$(cat "${commands_dir}/hefaos_fmt.status" 2>/dev/null || true)"
    printf 'hefaos_clippy_workspace_status=%s\n' "$(cat "${commands_dir}/hefaos_clippy_workspace.status" 2>/dev/null || true)"
    printf 'hefaos_test_workspace_status=%s\n' "$(cat "${commands_dir}/hefaos_test_workspace.status" 2>/dev/null || true)"
    printf '%s\n' 'iceoryx2_control_admission=rejected: bincode Vec copy and no declared queue/schema/epoch/pool policy'
} >"${evidence_root}/verdict.txt"; then
    printf '%s\n' "error: cannot write evidence verdict" >&2
    exit 1
fi

if [[ "$(head -n1 "${commands_dir}/upstream_revision.stdout" 2>/dev/null || true)" != "${copper_revision}" ]]; then
    failed=1
fi
if [[ "${failed}" -eq 0 ]]; then
    printf '%s\n' 'hefaos_spike=not-accepted: raw bundle requires a committed or reviewed linked evidence record' >>"${evidence_root}/verdict.txt" || exit 1
    exit 1
else
    printf '%s\n' 'hefaos_spike=fail' >>"${evidence_root}/verdict.txt" || exit 1
fi
exit "${failed}"
