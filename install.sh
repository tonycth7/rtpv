#!/usr/bin/env bash
# rtpv installer
# Usage: curl -fsSL https://raw.githubusercontent.com/tonycth7/rtpv/main/install.sh | bash
set -euo pipefail

REPO="https://github.com/tonycth7/rtpv"
BIN_DIR="${RTPV_BIN_DIR:-$HOME/.local/bin}"
INSTALL_TUI="${RTPV_INSTALL_TUI:-true}"
INSTALL_MENU="${RTPV_INSTALL_MENU:-true}"

# ── colors ───────────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  C_GREEN='\033[38;5;151m' C_CYAN='\033[38;5;117m'
  C_YELLOW='\033[38;5;222m' C_RED='\033[38;5;203m'
  C_BOLD='\033[1m' C_DIM='\033[2m' C_RESET='\033[0m'
else
  C_GREEN='' C_CYAN='' C_YELLOW='' C_RED='' C_BOLD='' C_DIM='' C_RESET=''
fi

ok()   { printf "${C_GREEN}${C_BOLD}  ✔  %s${C_RESET}\n" "$1"; }
info() { printf "${C_CYAN}  ℹ  %s${C_RESET}\n" "$1"; }
step() { printf "${C_CYAN}  →  %s${C_RESET}\n" "$1"; }
warn() { printf "${C_YELLOW}  ⚠  %s${C_RESET}\n" "$1" >&2; }
die()  { printf "${C_RED}${C_BOLD}  ✖  %s${C_RESET}\n" "$1" >&2; exit 1; }

# ── banner ───────────────────────────────────────────────────────────────────
printf "\n${C_BOLD}${C_CYAN}"
printf "  ╔══════════════════════════════════════════╗\n"
printf "  ║          rtpv  installer                 ║\n"
printf "  ╚══════════════════════════════════════════╝\n"
printf "${C_RESET}\n"

# ── check Rust ───────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  warn "Rust not found — installing via rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
  # shellcheck source=/dev/null
  source "$HOME/.cargo/env"
fi
ok "Rust $(rustc --version | cut -d' ' -f2)"

# ── create bin dir ───────────────────────────────────────────────────────────
mkdir -p "$BIN_DIR"
info "Installing to $BIN_DIR"

# ── build ────────────────────────────────────────────────────────────────────
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

step "Cloning rtpv..."
git clone --depth 1 "$REPO" "$TMPDIR/rtpv"
cd "$TMPDIR/rtpv"

step "Building rtpv (release)..."
cargo build --release -p rtpv

if [ "$INSTALL_TUI" = "true" ]; then
  step "Building rtpv-tui..."
  cargo build --release -p rtpv-tui
fi

if [ "$INSTALL_MENU" = "true" ]; then
  step "Building rtpv-menu..."
  cargo build --release -p rtpv-menu
fi

# ── install binaries ─────────────────────────────────────────────────────────
step "Installing binaries..."
install -m 755 target/release/rtpv "$BIN_DIR/rtpv"
ok "Installed rtpv → $BIN_DIR/rtpv"

if [ "$INSTALL_TUI" = "true" ] && [ -f target/release/rtpv-tui ]; then
  install -m 755 target/release/rtpv-tui "$BIN_DIR/rtpv-tui"
  ok "Installed rtpv-tui → $BIN_DIR/rtpv-tui"
fi

if [ "$INSTALL_MENU" = "true" ] && [ -f target/release/rtpv-menu ]; then
  install -m 755 target/release/rtpv-menu "$BIN_DIR/rtpv-menu"
  ok "Installed rtpv-menu → $BIN_DIR/rtpv-menu"
fi

# ── PATH check ───────────────────────────────────────────────────────────────
if ! echo "$PATH" | grep -q "$BIN_DIR"; then
  warn "$BIN_DIR is not in PATH"
  printf "  ${C_DIM}Add this to ~/.bashrc / ~/.zshrc:${C_RESET}\n"
  printf "  ${C_DIM}  export PATH=\"\$PATH:%s\"${C_RESET}\n\n" "$BIN_DIR"
fi

# ── init store ───────────────────────────────────────────────────────────────
if [ ! -f "$HOME/.config/rtpv/identity.age" ]; then
  printf "\n"
  info "No rtpv store found — initializing..."
  "$BIN_DIR/rtpv" init
fi

# ── done ─────────────────────────────────────────────────────────────────────
printf "\n${C_BOLD}${C_GREEN}  rtpv installed successfully!${C_RESET}\n\n"
printf "  ${C_DIM}Quick start:${C_RESET}\n"
printf "    rtpv add github\n"
printf "    rtpv copy github\n"
printf "    rtpv-tui          # built-in picker\n"
printf "    rtpv-menu         # WM launcher\n"
printf "    rtpv --help\n\n"
