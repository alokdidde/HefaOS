#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_dir}/../.." && pwd)"
model_lock="${repository_root}/testbench/so101/model.lock.toml"

toml_string() {
  local section="$1"
  local key="$2"
  awk -v wanted_section="${section}" -v wanted_key="${key}" '
    $0 == "[" wanted_section "]" { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && $0 ~ "^" wanted_key " = \"" {
      value = $0
      sub("^" wanted_key " = \"", "", value)
      sub("\"$", "", value)
      print value
      exit
    }
  ' "${model_lock}"
}

readonly MENAGERIE_URL="$(toml_string source repository)"
readonly MENAGERIE_COMMIT="$(toml_string source commit)"
readonly MENAGERIE_DIRECTORY="$(toml_string source directory)"
readonly MENAGERIE_TREE="$(toml_string source directory_git_tree_sha1)"
readonly MUJOCO_VERSION="$(toml_string engine mujoco_version)"

if [[ -z "${MENAGERIE_URL}" || -z "${MENAGERIE_COMMIT}" || -z "${MENAGERIE_DIRECTORY}" || -z "${MENAGERIE_TREE}" || -z "${MUJOCO_VERSION}" ]]; then
  echo "error: ${model_lock} is missing a required source or engine lock value" >&2
  exit 1
fi

cache_root="${repository_root}/testbench/.cache"
menagerie_dir="${cache_root}/mujoco-menagerie"
mujoco_download_dir="${cache_root}/mujoco"
model_dir="${menagerie_dir}/${MENAGERIE_DIRECTORY}"

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
git -C "${menagerie_dir}" sparse-checkout set "${MENAGERIE_DIRECTORY}"
git -C "${menagerie_dir}" checkout --detach "${MENAGERIE_COMMIT}"

actual_commit="$(git -C "${menagerie_dir}" rev-parse HEAD)"
actual_tree="$(git -C "${menagerie_dir}" rev-parse "HEAD:${MENAGERIE_DIRECTORY}")"

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
export LD_LIBRARY_PATH="${mujoco_download_dir}/mujoco-${MUJOCO_VERSION}/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"

cd "${repository_root}"

if [[ "$#" -eq 0 ]]; then
  exec cargo test --locked -p hefaos-testbench-so101 --features mujoco
fi

exec "$@"
