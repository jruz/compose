#!/usr/bin/env bash
set -euo pipefail

OS=$1
ARCH=$2
TARGET="${ARCH}-${OS}"

rustup target add "$TARGET"
cargo zigbuild --release --target "$TARGET"
NAME=$(grep '^name' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
cp "target/${TARGET}/release/${NAME}" "${NAME}-${OS}-${ARCH}"
