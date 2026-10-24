#!/usr/bin/env bash
# Build the AppImage and the Snap from the payload staged by package_release.sh.
# Usage: package_formats.sh [appimage] [snap]   (default: both)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION="$(grep -E '^version\s*=' "${PROJECT_ROOT}/Cargo.toml" | head -n1 | cut -d'"' -f2)"
ARCH="x86_64"
DIST_DIR="${PROJECT_ROOT}/dist"
STAGE_DIR="${DIST_DIR}/cutlery-v${VERSION}-linux-${ARCH}"
PACKAGING="${PROJECT_ROOT}/packaging"

# Pinned AppImage tooling, verified before use.
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
APPIMAGETOOL_SHA256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
RUNTIME_SHA256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

if [ ! -x "${STAGE_DIR}/cutlery" ]; then
    echo "Release payload not found in ${STAGE_DIR}; run scripts/package_release.sh first." >&2
    exit 1
fi

TARGETS=("$@")
[ ${#TARGETS[@]} -eq 0 ] && TARGETS=(appimage snap)

fetch_verified() {
    local url="$1" sha="$2" dest="$3"
    if [ ! -f "${dest}" ] || ! echo "${sha}  ${dest}" | sha256sum -c --status; then
        curl -fsSL --retry 3 -o "${dest}" "${url}"
        echo "${sha}  ${dest}" | sha256sum -c --status || { echo "SHA-256 mismatch for ${url}" >&2; rm -f "${dest}"; exit 1; }
    fi
}

build_appimage() {
    local tools="${DIST_DIR}/tools" appdir="${DIST_DIR}/Cutlery.AppDir"
    local out="${DIST_DIR}/Cutlery-v${VERSION}-${ARCH}.AppImage"
    mkdir -p "${tools}"
    fetch_verified "${APPIMAGETOOL_URL}" "${APPIMAGETOOL_SHA256}" "${tools}/appimagetool"
    fetch_verified "${RUNTIME_URL}" "${RUNTIME_SHA256}" "${tools}/runtime-x86_64"
    chmod +x "${tools}/appimagetool"

    rm -rf "${appdir}"
    cp -a "${STAGE_DIR}" "${appdir}"
    install -m 755 "${PACKAGING}/appimage/AppRun" "${appdir}/AppRun"
    cp "${PACKAGING}/appimage/cutlery.desktop" "${appdir}/cutlery.desktop"
    cp "${PROJECT_ROOT}/assets/icons/cutlery-256x256.png" "${appdir}/cutlery.png"
    ln -sf cutlery.png "${appdir}/.DirIcon"

    # appimagetool is itself an AppImage; extract-and-run avoids needing FUSE on the build host.
    rm -f "${out}"
    APPIMAGE_EXTRACT_AND_RUN=1 ARCH="${ARCH}" "${tools}/appimagetool" --no-appstream \
        --runtime-file "${tools}/runtime-x86_64" "${appdir}" "${out}"
    (cd "${DIST_DIR}" && sha256sum "$(basename "${out}")" > "$(basename "${out}").sha256")
    rm -rf "${appdir}"
    echo "AppImage: ${out}"
}

build_snap() {
    local project="${DIST_DIR}/snap-project"
    rm -rf "${project}"
    mkdir -p "${project}/snap/gui"
    sed "s/@VERSION@/${VERSION}/" "${PACKAGING}/snap/snapcraft.yaml" > "${project}/snap/snapcraft.yaml"
    cp "${PACKAGING}/snap/gui/cutlery.desktop" "${project}/snap/gui/cutlery.desktop"
    cp "${PROJECT_ROOT}/assets/icons/cutlery-256x256.png" "${project}/snap/gui/cutlery.png"
    cp -a "${STAGE_DIR}" "${project}/payload"

    # CI runs on Ubuntu 24.04, which matches the core24 base, so no build VM is needed.
    (cd "${project}" && snapcraft pack --destructive-mode --output "${DIST_DIR}/cutlery_${VERSION}_amd64.snap")
    (cd "${DIST_DIR}" && sha256sum "cutlery_${VERSION}_amd64.snap" > "cutlery_${VERSION}_amd64.snap.sha256")
    rm -rf "${project}"
    echo "Snap: ${DIST_DIR}/cutlery_${VERSION}_amd64.snap"
}

for target in "${TARGETS[@]}"; do
    case "${target}" in
        appimage) build_appimage ;;
        snap) build_snap ;;
        *) echo "Unknown target: ${target}" >&2; exit 1 ;;
    esac
done
