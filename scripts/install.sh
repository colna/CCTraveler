#!/usr/bin/env bash
# CCTraveler 一行安装脚本
#   curl -fsSL https://raw.githubusercontent.com/colna/CCTraveler/main/scripts/install.sh | sh
#
# 或指定版本：
#   CCTRAVELER_VERSION=v0.4.0 curl ... | sh
#
# 行为：
#   1. 探测 OS / arch，选定对应预编译 tarball
#   2. 从 GitHub Releases 下载并校验
#   3. 解压到 $CCTRAVELER_INSTALL_DIR (默认 ~/.local/bin)
#   4. 提示加 PATH
set -euo pipefail

REPO="colna/CCTraveler"
VERSION="${CCTRAVELER_VERSION:-latest}"
INSTALL_DIR="${CCTRAVELER_INSTALL_DIR:-$HOME/.local/bin}"

red()   { printf "\033[31m%s\033[0m\n" "$*"; }
green() { printf "\033[32m%s\033[0m\n" "$*"; }
blue()  { printf "\033[34m%s\033[0m\n" "$*"; }

detect_target() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os-$arch" in
    darwin-arm64)        echo "aarch64-apple-darwin" ;;
    darwin-x86_64)       echo "x86_64-apple-darwin" ;;
    linux-x86_64)        echo "x86_64-unknown-linux-gnu" ;;
    linux-aarch64|linux-arm64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      red "不支持的平台: $os-$arch"
      red "请走 cargo install cctraveler 或自行编译。"
      exit 1
      ;;
  esac
}

resolve_version() {
  if [ "$VERSION" != "latest" ]; then
    echo "$VERSION"
    return
  fi
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
      | grep -oE '"tag_name":\s*"[^"]+"' \
      | head -n1 \
      | sed -E 's/.*"([^"]+)"$/\1/'
  else
    red "需要 curl 才能解析最新版本。"
    exit 1
  fi
}

main() {
  blue "CCTraveler installer"
  local target version url tmp
  target="$(detect_target)"
  version="$(resolve_version)"
  if [ -z "$version" ]; then
    red "无法解析最新版本号；可设置 CCTRAVELER_VERSION=v0.x.y 重试。"
    exit 1
  fi
  blue "  target  : $target"
  blue "  version : $version"
  blue "  install : $INSTALL_DIR"

  url="https://github.com/${REPO}/releases/download/${version}/cctraveler-${version}-${target}.tar.gz"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  blue "下载 $url"
  if ! curl -fL -o "$tmp/cct.tar.gz" "$url"; then
    red "下载失败。请检查 release 资产是否存在。"
    exit 1
  fi

  tar -xzf "$tmp/cct.tar.gz" -C "$tmp"
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$tmp/cctraveler" "$INSTALL_DIR/cctraveler"

  green "✔ 已安装到 $INSTALL_DIR/cctraveler"

  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      echo
      blue "提示：$INSTALL_DIR 不在 PATH 中。请把以下行加到 shell 配置（~/.zshrc 或 ~/.bashrc）："
      printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR"
      ;;
  esac

  echo
  green "现在运行 cctraveler init 初始化，然后输入 cctraveler 进入对话。"
}

main "$@"
