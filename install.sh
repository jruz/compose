#!/bin/sh
set -eu

REPO="jruz/compose"
BASE_URL="https://github.com/${REPO}/releases"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_platform() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Linux)  os="unknown-linux-musl" ;;
    Darwin) os="apple-darwin" ;;
    *)
      echo "ERROR: unsupported OS: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64)       arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *)
      echo "ERROR: unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  echo "${os}-${arch}"
}

fetch_latest_tag() {
  url="https://api.github.com/repos/${REPO}/releases/latest"
  tag=$(curl -fsSL "$url" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
  if [ -z "$tag" ]; then
    echo "ERROR: could not fetch latest release tag" >&2
    exit 1
  fi
  echo "$tag"
}

main() {
  platform=$(detect_platform)
  tag=$(fetch_latest_tag)
  binary="compose-${platform}"
  url="${BASE_URL}/download/${tag}/${binary}"

  echo "Installing compose ${tag} (${platform})..."
  echo "  from: ${url}"
  echo "  to:   ${INSTALL_DIR}/compose"

  mkdir -p "$INSTALL_DIR"
  curl -fsSL "$url" -o "${INSTALL_DIR}/compose"
  chmod +x "${INSTALL_DIR}/compose"

  echo "Installed compose ${tag} to ${INSTALL_DIR}/compose"

  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo ""
    echo "NOTE: ${INSTALL_DIR} is not in your PATH."
    echo "  Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
}

main
