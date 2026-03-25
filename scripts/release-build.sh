#!/usr/bin/env bash
set -euo pipefail

OS=$1
ARCH=$2
TARGET="${ARCH}-${OS}"
NAME=$(grep '^name' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

docker run \
  --rm \
  --volume "$(pwd):/workspace" \
  --workdir /workspace \
  --env REVISION="${REVISION}" \
  ghcr.io/rust-cross/cargo-zigbuild:0.21.4 \
  bash -c "rustup target add ${TARGET} && cargo zigbuild --release --target ${TARGET}"

cp "target/${TARGET}/release/${NAME}" "${NAME}-${OS}-${ARCH}"
