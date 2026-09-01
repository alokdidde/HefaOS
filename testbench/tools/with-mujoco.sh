#!/usr/bin/env bash

set -euo pipefail

readonly MENAGERIE_URL="https://github.com/google-deepmind/mujoco_menagerie.git"
readonly MENAGERIE_COMMIT="da76818e269b82289eba39808e2fb91d679d6994"
readonly MENAGERIE_TREE="76a095b3fec789ca460f4b8ceca66d11ba96040c"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_dir}/../.." && pwd)"
cache_root="${repository_root}/testbench/.cache"
menagerie_dir="${cache_root}/mujoco-menagerie"
mujoco_download_dir="${cache_root}/mujoco"
model_dir="${menagerie_dir}/robotstudio_so101"

mkdir -p "${cache_root}" "${mujoco_download_dir}"

if [[ -e "${menagerie_dir}" && ! -d "${menagerie_dir}/.git" ]]; then
  echo "error: ${menagerie_dir} exists but is not a Git checkout" >&2
  exit 1
fi

new_checkout=0
if [[ ! -d "${menagerie_dir}/.git" ]]; then
  git clone --filter=blob:none --no-checkout "${MENAGERIE_URL}" "${menagerie_dir}"
  new_checkout=1
fi

if [[ "${new_checkout}" -eq 0 && -n "$(git -C "${menagerie_dir}" status --porcelain)" ]]; then
  echo "error: pinned Menagerie cache has local modifications: ${menagerie_dir}" >&2
  exit 1
fi

git -C "${menagerie_dir}" fetch --depth 1 origin "${MENAGERIE_COMMIT}"
git -C "${menagerie_dir}" sparse-checkout init --cone
git -C "${menagerie_dir}" sparse-checkout set robotstudio_so101
git -C "${menagerie_dir}" checkout --detach "${MENAGERIE_COMMIT}"

actual_commit="$(git -C "${menagerie_dir}" rev-parse HEAD)"
actual_tree="$(git -C "${menagerie_dir}" rev-parse HEAD:robotstudio_so101)"

if [[ "${actual_commit}" != "${MENAGERIE_COMMIT}" ]]; then
  echo "error: expected Menagerie commit ${MENAGERIE_COMMIT}, got ${actual_commit}" >&2
  exit 1
fi

if [[ "${actual_tree}" != "${MENAGERIE_TREE}" ]]; then
  echo "error: expected SO-101 tree ${MENAGERIE_TREE}, got ${actual_tree}" >&2
  exit 1
fi

(
  cd "${model_dir}"
  sha256sum --check --status "${repository_root}/testbench/so101/execution-files.sha256"
)

export HEFAOS_SO101_MODEL_DIR="${model_dir}"
export MUJOCO_DOWNLOAD_DIR="${mujoco_download_dir}"
export MUJOCO_NO_PKG_CONFIG=1
export LD_LIBRARY_PATH="${mujoco_download_dir}/mujoco-3.9.0/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"

cd "${repository_root}"

if [[ "$#" -eq 0 ]]; then
  exec cargo test --locked -p hefaos-testbench-so101 --features mujoco
fi

exec "$@"
