#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SANDBOX_DIR="${PROJECT_ROOT}/sandbox"
PREFIX_DIR="${SANDBOX_DIR}/prefix"
DATA_DIR="${PROJECT_ROOT}/data"

export WINEPREFIX="${PREFIX_DIR}"
export WINEARCH="win64"
export WINEDEBUG="-all"
export WINEFSYNC=1
export WINENTSYNC=1
export WINEDLLOVERRIDES="winegstreamer=d;mscoree=d;mshtml=d"

echo "=== [1/3] Initializing WINEPREFIX at: ${PREFIX_DIR} ==="
mkdir -p "${PREFIX_DIR}"
wineboot -u

echo "=== [2/3] Applying Registry Tweaks (Docking & CSMT) ==="
if [ -f "${DATA_DIR}/x11_docking_fix.reg" ]; then
    wine regedit "${DATA_DIR}/x11_docking_fix.reg"
elif [ -f "${SANDBOX_DIR}/x11_docking_fix.reg" ]; then
    wine regedit "${SANDBOX_DIR}/x11_docking_fix.reg"
fi

echo "=== [3/3] Downloading and Installing Roblox Studio Packages ==="
chmod +x "${SCRIPT_DIR}/install_studio.py"
python3 "${SCRIPT_DIR}/install_studio.py"

echo ""
echo "================================================================="
echo " Roblox Studio installation completed successfully!"
echo " To start Roblox Studio with all optimizations, run:"
echo "   cutlery run"
echo "================================================================="

