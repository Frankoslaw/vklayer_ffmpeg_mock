#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
MOONSHINE_DIR="${ROOT_DIR}/../moonshine"
CONFIG_PATH="${1:-${HOME}/.config/moonshine/config.toml}"

if [[ ! -d "${MOONSHINE_DIR}" ]]; then
    echo "Moonshine source checkout not found at ${MOONSHINE_DIR}" >&2
    exit 1
fi
if [[ ! -f "${CONFIG_PATH}" ]]; then
    echo "Moonshine config not found at ${CONFIG_PATH}" >&2
    exit 1
fi

cargo build --release --manifest-path "${ROOT_DIR}/Cargo.toml"
export VK_LAYER_PATH="${ROOT_DIR}${VK_LAYER_PATH:+:${VK_LAYER_PATH}}"
export VK_INSTANCE_LAYERS="VK_LAYER_FFMPEG_mock${VK_INSTANCE_LAYERS:+:${VK_INSTANCE_LAYERS}}"

cd "${MOONSHINE_DIR}"
cargo run --release -- "${CONFIG_PATH}"
