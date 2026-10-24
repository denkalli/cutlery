#!/usr/bin/env python3
"""
Native Roblox Studio Package Downloader & Installer for Linux/Wine.
Bypasses the deprecated/broken Windows bootstrapper (RobloxStudioLauncherBeta.exe)
by fetching packages directly from Roblox CDN and extracting them deterministically.
"""

import os
import sys
import json
import hashlib
import getpass
import urllib.request
import zipfile
import tarfile
import io
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

CDN_BASE = "https://setup.rbxcdn.com"
VERSION_API = "https://clientsettingscdn.roblox.com/v2/client-version/WindowsStudio64"
DXVK_VERSION = "3.1.1"
DXVK_URL = f"https://github.com/doitsujin/dxvk/releases/download/v{DXVK_VERSION}/dxvk-{DXVK_VERSION}.tar.gz"
DXVK_SHA256 = "40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608"
HTTP_TIMEOUT = 30
# Read by the Cutlery UI to show the human-readable Studio version.
STUDIO_VERSION_MARKER = "cutlery-studio-version.txt"

# Official package to subdirectory mapping inside Roblox Studio directory
PACKAGE_DIRS = {
    "ApplicationConfig.zip": "ApplicationConfig",
    "redist.zip": "",
    "RobloxStudio.zip": "",
    "Libraries.zip": "",
    "content-avatar.zip": "content/avatar",
    "content-configs.zip": "content/configs",
    "content-fonts.zip": "content/fonts",
    "content-sky.zip": "content/sky",
    "content-sounds.zip": "content/sounds",
    "content-textures2.zip": "content/textures",
    "content-studio_svg_textures.zip": "content/studio_svg_textures",
    "content-models.zip": "content/models",
    "content-textures3.zip": "PlatformContent/pc/textures",
    "content-terrain.zip": "PlatformContent/pc/terrain",
    "content-platform-fonts.zip": "PlatformContent/pc/fonts",
    "content-platform-dictionaries.zip": "PlatformContent/pc/shared_compression_dictionaries",
    "content-qt_translations.zip": "content/qt_translations",
    "content-api-docs.zip": "content/api_docs",
    "extracontent-scripts.zip": "ExtraContent/scripts",
    "extracontent-luapackages.zip": "ExtraContent/LuaPackages",
    "extracontent-translations.zip": "ExtraContent/translations",
    "extracontent-models.zip": "ExtraContent/models",
    "extracontent-textures.zip": "ExtraContent/textures",
    "studiocontent-models.zip": "StudioContent/models",
    "studiocontent-textures.zip": "StudioContent/textures",
    "shaders.zip": "shaders",
    "BuiltInPlugins.zip": "BuiltInPlugins",
    "BuiltInStandalonePlugins.zip": "BuiltInStandalonePlugins",
    "LibrariesQt5.zip": "",
    "Plugins.zip": "Plugins",
    "StudioFonts.zip": "StudioFonts",
    "ssl.zip": "ssl",
    "WebView2.zip": "",
    "WebView2RuntimeInstaller.zip": "WebView2RuntimeInstaller"
}

def get_latest_version():
    print("[1/5] Checking latest Roblox Studio version...")
    req = urllib.request.Request(VERSION_API, headers={"User-Agent": "Roblox/Linux"})
    with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as resp:
        data = json.loads(resp.read().decode())
    version_guid = data.get("clientVersionUpload")
    version_id = data.get("version")
    print(f"      Detected version: {version_id} ({version_guid})")
    return version_guid, version_id

def fetch_manifest(version_guid):
    manifest_url = f"{CDN_BASE}/{version_guid}-rbxPkgManifest.txt"
    print(f"[2/5] Fetching package manifest: {manifest_url}")
    req = urllib.request.Request(manifest_url, headers={"User-Agent": "Roblox/Linux"})
    with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as resp:
        content = resp.read().decode("utf-8")
    
    lines = [line.strip() for line in content.splitlines() if line.strip()]
    if not lines or lines[0] != "v0":
        raise ValueError("Invalid manifest format.")
    
    packages = []
    for i in range(1, len(lines), 4):
        pkg_name = lines[i]
        checksum = lines[i+1]
        size = int(lines[i+2])
        packages.append((pkg_name, checksum, size))
    
    print(f"      {len(packages)} packages listed in manifest.")
    return packages

def download_and_extract_package(pkg_name, expected_checksum, expected_size, version_guid, target_base_dir):
    rel_dest = PACKAGE_DIRS.get(pkg_name, "")
    dest_dir = (target_base_dir / rel_dest).resolve()
    dest_dir.mkdir(parents=True, exist_ok=True)
    
    pkg_url = f"{CDN_BASE}/{version_guid}-{pkg_name}"
    req = urllib.request.Request(pkg_url, headers={"User-Agent": "Roblox/Linux"})
    
    with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as resp:
        zip_data = resp.read()
    
    # 1. Integrity check: Byte size
    if expected_size and len(zip_data) != expected_size:
        raise ValueError(
            f"Integrity check failed for {pkg_name}: size mismatch (got {len(zip_data)}, expected {expected_size})"
        )
    
    # 2. Integrity check: MD5 Checksum
    if expected_checksum:
        actual_md5 = hashlib.md5(zip_data).hexdigest()
        if actual_md5.lower() != expected_checksum.lower():
            raise ValueError(
                f"Integrity check failed for {pkg_name}: MD5 mismatch (got {actual_md5}, expected {expected_checksum})"
            )
    
    # 3. Zip extraction with Zip Slip path traversal protection
    with zipfile.ZipFile(io.BytesIO(zip_data)) as z:
        for member in z.infolist():
            clean_filename = member.filename.replace("\\", "/").lstrip("/")
            if not clean_filename or clean_filename.endswith("/"):
                continue
            
            target_path = (dest_dir / clean_filename).resolve()
            try:
                target_path.relative_to(dest_dir)
            except ValueError:
                raise ValueError(
                    f"Security violation: Zip Slip path traversal detected in '{pkg_name}' for entry '{member.filename}'"
                )
            
            target_path.parent.mkdir(parents=True, exist_ok=True)
            
            with z.open(member) as src_file, open(target_path, "wb") as dst_file:
                dst_file.write(src_file.read())
    
    return pkg_name

def main():
    script_dir = Path(__file__).resolve().parent
    project_root = script_dir.parent
    sandbox_dir = Path(os.environ.get("CUTLERY_SANDBOX") or project_root / "sandbox")
    data_dir = project_root / "data"
    prefix_dir = Path(os.environ.get("CUTLERY_PREFIX") or sandbox_dir / "prefix")
    
    # Locate Windows user directory inside WINEPREFIX
    users_dir = prefix_dir / "drive_c" / "users"
    if not users_dir.exists():
        print("Error: WINEPREFIX not initialized yet. Please run wineboot -u first.")
        sys.exit(1)
    
    current_user = os.environ.get("USER") or getpass.getuser()
    if not (users_dir / current_user).exists():
        candidates = [
            u for u in users_dir.iterdir()
            if u.is_dir() and u.name not in ("Public", "Default", "default", "All Users")
        ]
        if candidates:
            current_user = candidates[0].name
    
    user_appdata = users_dir / current_user / "AppData" / "Local" / "Roblox" / "Versions"
    user_appdata.mkdir(parents=True, exist_ok=True)
    
    version_guid, version_id = get_latest_version()
    version_dir = user_appdata / version_guid
    version_dir.mkdir(parents=True, exist_ok=True)
    
    packages = fetch_manifest(version_guid)
    (version_dir / STUDIO_VERSION_MARKER).write_text(f"{version_id}\n")
    
    print(f"[3/5] Downloading, verifying integrity, and extracting {len(packages)} packages in parallel...")
    completed = 0
    with ThreadPoolExecutor(max_workers=8) as executor:
        futures = {
            executor.submit(
                download_and_extract_package,
                pkg[0],
                pkg[1],
                pkg[2],
                version_guid,
                version_dir,
            ): pkg[0]
            for pkg in packages
        }
        for future in as_completed(futures):
            pkg_name = futures[future]
            try:
                future.result()
                completed += 1
                sys.stdout.write(f"\r      Progress: [{completed}/{len(packages)}] {pkg_name[:30]:<30}")
                sys.stdout.flush()
            except Exception as e:
                print(f"\n[Failed] Package {pkg_name}: {e}")
                raise
    
    print("\n[4/5] Injecting ClientAppSettings.json and dxvk.conf...")
    settings_dir = version_dir / "ClientSettings"
    settings_dir.mkdir(parents=True, exist_ok=True)
    
    settings_src = data_dir / "ClientAppSettings.json"
    if not settings_src.exists():
        settings_src = sandbox_dir / "ClientAppSettings.json"
    if settings_src.exists():
        with open(settings_src, "r") as src, open(settings_dir / "ClientAppSettings.json", "w") as dst:
            dst.write(src.read())
        print(f"      FastFlags injected into {settings_dir / 'ClientAppSettings.json'}")
    
    # Generate required AppSettings.xml
    app_settings_xml = version_dir / "AppSettings.xml"
    with open(app_settings_xml, "w", encoding="utf-8") as f:
        f.write('<?xml version="1.0" encoding="UTF-8"?>\n<Settings>\n  <ContentFolder>content</ContentFolder>\n  <BaseUrl>https://www.roblox.com</BaseUrl>\n</Settings>\n')
    print(f"      AppSettings.xml generated at {app_settings_xml}")

    dxvk_src = data_dir / "dxvk.conf"
    if not dxvk_src.exists():
        dxvk_src = sandbox_dir / "dxvk.conf"
    if dxvk_src.exists():
        with open(dxvk_src, "r") as src, open(version_dir / "dxvk.conf", "w") as dst:
            dst.write(src.read())
        print(f"      dxvk.conf copied to version root.")
    
    # Inject DXVK binaries (downloading automatically with SHA256 integrity check if not cached)
    dxvk_x64 = sandbox_dir / "dxvk" / "x64"
    if not (dxvk_x64 / "d3d11.dll").exists():
        if (data_dir / "dxvk" / "x64" / "d3d11.dll").exists():
            dxvk_x64 = data_dir / "dxvk" / "x64"
        else:
            print(f"      Downloading DXVK v{DXVK_VERSION} from {DXVK_URL}...")
            req = urllib.request.Request(DXVK_URL, headers={"User-Agent": "Cutlery/Linux"})
            with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as resp:
                tar_bytes = resp.read()
            
            actual_dxvk_sha256 = hashlib.sha256(tar_bytes).hexdigest()
            if actual_dxvk_sha256.lower() != DXVK_SHA256.lower():
                raise ValueError(
                    f"Integrity check failed: DXVK tarball SHA256 mismatch (got {actual_dxvk_sha256}, expected {DXVK_SHA256})"
                )
            
            dxvk_x64.mkdir(parents=True, exist_ok=True)
            with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:gz") as tar:
                for member in tar.getmembers():
                    if "/x64/" in member.name and member.isfile():
                        dll_filename = Path(member.name).name
                        f_obj = tar.extractfile(member)
                        if f_obj:
                            with open(dxvk_x64 / dll_filename, "wb") as out_f:
                                out_f.write(f_obj.read())
            (dxvk_x64.parent / "VERSION").write_text(f"{DXVK_VERSION}\n")

    if dxvk_x64.exists():
        for dll_name in ["d3d11.dll", "dxgi.dll"]:
            dll_src = dxvk_x64 / dll_name
            if dll_src.exists():
                with open(dll_src, "rb") as s, open(version_dir / dll_name, "wb") as d:
                    d.write(s.read())
        print(f"      DXVK DLLs (d3d11.dll, dxgi.dll) injected into version root.")
        
    print(f"[5/5] Installation completed successfully!")
    print(f"      Executable: {version_dir / 'RobloxStudioBeta.exe'}")

if __name__ == "__main__":
    main()
