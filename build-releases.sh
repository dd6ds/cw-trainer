#!/usr/bin/env bash
# build-releases.sh — cross-compile cw-trainer for all platforms
# Auto-installs missing build tools (cross, cargo-zigbuild, zig).
# macOS targets additionally require the macOS SDK in your osxcross/zig setup.

set -euo pipefail

BINARY="cw-trainer"
RELEASES_DIR="releases"

# ── colours ──────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m';  BOLD='\033[1m';   NC='\033[0m'

info()  { echo -e "${CYAN}${BOLD}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}${BOLD}[ OK ]${NC}  $*"; }
warn()  { echo -e "${YELLOW}${BOLD}[WARN]${NC}  $*"; }
fail()  { echo -e "${RED}${BOLD}[FAIL]${NC}  $*"; }
step()  { echo -e "${BOLD}── $* ──────────────────────────${NC}"; }

# ── dependency installer ──────────────────────────────────────────────────────
ensure_cross() {
    if command -v cross &>/dev/null; then
        ok "cross already installed ($(cross --version 2>&1 | head -1))"
        return
    fi
    info "Installing cross…"
    cargo install cross --git https://github.com/cross-rs/cross
    ok "cross installed"
}

ensure_zigbuild() {
    if command -v cargo-zigbuild &>/dev/null; then
        ok "cargo-zigbuild already installed"
    else
        info "Installing cargo-zigbuild…"
        cargo install cargo-zigbuild
        ok "cargo-zigbuild installed"
    fi

    if command -v zig &>/dev/null; then
        ok "zig already installed ($(zig version))"
        return
    fi

    info "Installing zig…"
    # Try snap first, then apt, then direct download
    if command -v snap &>/dev/null; then
        sudo snap install zig --classic --channel 0.13
    elif command -v apt-get &>/dev/null; then
        sudo apt-get update -qq
        sudo apt-get install -y wget xz-utils
        ZIG_VER="0.13.0"
        ZIG_TAR="zig-linux-x86_64-${ZIG_VER}.tar.xz"
        wget -q "https://ziglang.org/download/${ZIG_VER}/${ZIG_TAR}"
        sudo tar -xf "$ZIG_TAR" -C /usr/local/
        sudo ln -sf "/usr/local/zig-linux-x86_64-${ZIG_VER}/zig" /usr/local/bin/zig
        rm -f "$ZIG_TAR"
    else
        fail "Cannot install zig automatically — please install manually: https://ziglang.org/download/"
        exit 1
    fi
    ok "zig installed ($(zig version))"
}

ensure_docker() {
    if ! command -v docker &>/dev/null; then
        fail "Docker is required for cross but is not installed."
        info "Install Docker: https://docs.docker.com/engine/install/"
        exit 1
    fi
    if ! docker info &>/dev/null 2>&1; then
        fail "Docker is installed but not running. Please start Docker and retry."
        exit 1
    fi
    ok "Docker is running"
}

ensure_rustup_target() {
    local target="$1"
    if ! rustup target list --installed | grep -q "^${target}$"; then
        info "Adding rustup target: $target"
        rustup target add "$target"
    fi
}

# ── target table: "rust-target|label|ext|tool" ───────────────────────────────
TARGETS=(
    "i686-pc-windows-gnu|windows-i686|.exe|cross"
    "x86_64-pc-windows-gnu|windows-amd64|.exe|cross"
    "i686-unknown-linux-gnu|linux-i686||cross"
    "x86_64-unknown-linux-gnu|linux-amd64||cross"
    "x86_64-apple-darwin|macos-x86_64||zigbuild"
    "aarch64-apple-darwin|macos-aarch64||zigbuild"
)

# ── preflight: install all needed tools ──────────────────────────────────────
echo
info "Checking / installing build dependencies…"
echo

ensure_docker
ensure_cross
ensure_zigbuild

echo

# ── clean stale host-compiled build scripts ───────────────────────────────────
# Cross containers have older glibc; host-compiled build scripts won't run
# inside them. A clean ensures everything is recompiled inside the container.
info "Cleaning stale build artifacts (avoids glibc mismatch inside containers)…"
cargo clean --release 2>/dev/null || true
echo

mkdir -p "$RELEASES_DIR"

# ── build loop ────────────────────────────────────────────────────────────────
BUILT=0; FAILED=0

for entry in "${TARGETS[@]}"; do
    IFS='|' read -r target label ext tool <<< "$entry"
    out="${RELEASES_DIR}/${BINARY}-${label}${ext}"

    step "$label  ($target)"

    ensure_rustup_target "$target"

    BUILD_OK=false
    if [[ "$tool" == "cross" ]]; then
        if cross build --release --target "$target" 2>&1; then
            BUILD_OK=true
        fi
    elif [[ "$tool" == "zigbuild" ]]; then
        if cargo zigbuild --release --target "$target" 2>&1; then
            BUILD_OK=true
        fi
    fi

    if [[ "$BUILD_OK" == "true" ]]; then
        src="target/${target}/release/${BINARY}${ext}"
        if [[ -f "$src" ]]; then
            cp "$src" "$out"
            size=$(du -sh "$out" | cut -f1)
            ok "$out  (${size})"
            (( BUILT++ )) || true
        else
            fail "Binary not found at $src"
            (( FAILED++ )) || true
        fi
    else
        fail "Build failed for $label"
        (( FAILED++ )) || true
    fi
    echo
done

# ── summary ──────────────────────────────────────────────────────────────────
echo -e "${BOLD}══ Summary ════════════════════════════════════════${NC}"
echo -e "  Built  : ${GREEN}${BUILT}${NC}"
echo -e "  Failed : ${RED}${FAILED}${NC}"
echo
if [[ $BUILT -gt 0 ]]; then
    echo -e "${BOLD}Releases:${NC}"
    ls -lh "$RELEASES_DIR/"
fi
