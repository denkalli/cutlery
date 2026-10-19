#!/usr/bin/env bash
# Package Cutlery standalone release archive and generate SHA-256 signatures.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION="${1:-$(grep -E '^version\s*=' "${PROJECT_ROOT}/Cargo.toml" | head -n1 | cut -d'"' -f2)}"
ARCH="x86_64"
DIST_NAME="cutlery-v${VERSION}-linux-${ARCH}"
DIST_DIR="${PROJECT_ROOT}/dist"
STAGE_DIR="${DIST_DIR}/${DIST_NAME}"

echo "=========================================================="
echo " Packaging Cutlery Release: v${VERSION} (${ARCH})"
echo "=========================================================="

# 1. Compile Cutlery binary in release mode
echo "[1/5] Building Cutlery binary with optimizations..."
cd "${PROJECT_ROOT}"
cargo build --release --locked

# 2. Compile X11 docking fix hooks
echo "[2/5] Compiling universal X11 docking hooks..."
make -C "${PROJECT_ROOT}/custom-libs" all

# 3. Prepare staging directory
echo "[3/5] Assembling release payload..."
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

# Binaries
cp "${PROJECT_ROOT}/target/release/cutlery" "${STAGE_DIR}/cutlery"
strip "${STAGE_DIR}/cutlery" 2>/dev/null || true

# Shared libraries & source
mkdir -p "${STAGE_DIR}/custom-libs/x86_64" "${STAGE_DIR}/custom-libs/i386"
cp "${PROJECT_ROOT}/custom-libs/wine_hook.so" "${STAGE_DIR}/custom-libs/"
cp "${PROJECT_ROOT}/custom-libs/x86_64/wine_hook.so" "${STAGE_DIR}/custom-libs/x86_64/"
cp "${PROJECT_ROOT}/custom-libs/i386/wine_hook.so" "${STAGE_DIR}/custom-libs/i386/"
cp "${PROJECT_ROOT}/custom-libs/wine_hook.c" "${STAGE_DIR}/custom-libs/"
cp "${PROJECT_ROOT}/custom-libs/Makefile" "${STAGE_DIR}/custom-libs/"

# Data, scripts, mcp, assets, plugins
cp -r "${PROJECT_ROOT}/data" "${STAGE_DIR}/data"
cp -r "${PROJECT_ROOT}/scripts" "${STAGE_DIR}/scripts"
# Remove internal pycache or staging scripts from payload
rm -rf "${STAGE_DIR}/scripts/__pycache__"
rm -f "${STAGE_DIR}/scripts/package_release.sh"

cp -r "${PROJECT_ROOT}/mcp" "${STAGE_DIR}/mcp"
rm -rf "${STAGE_DIR}/mcp/__pycache__"

cp -r "${PROJECT_ROOT}/assets" "${STAGE_DIR}/assets"
cp -r "${PROJECT_ROOT}/plugins" "${STAGE_DIR}/plugins"
cp "${PROJECT_ROOT}/LICENSE" "${STAGE_DIR}/LICENSE"
cp "${PROJECT_ROOT}/README.md" "${STAGE_DIR}/README.md"

# 4. Create compressed tarball
echo "[4/5] Compressing tarball archive..."
cd "${DIST_DIR}"
TARBALL="${DIST_NAME}.tar.gz"
tar -czf "${TARBALL}" "${DIST_NAME}"

# 5. Generate SHA-256 signature
echo "[5/5] Generating SHA-256 checksum..."
sha256sum "${TARBALL}" > "${TARBALL}.sha256"

echo "=========================================================="
echo " Package created successfully:"
echo "   Archive:  ${DIST_DIR}/${TARBALL}"
echo "   Checksum: $(cat "${DIST_DIR}/${TARBALL}.sha256")"
echo "=========================================================="
