#!/usr/bin/env python3
"""
Universal Roblox Studio MCP Bridge for Linux / Wine
Compatible with Claude Desktop, Cursor, Codex, Antigravity, and any standard MCP client.

Features:
- Dynamically discovers the latest StudioMCP.exe inside the active Wine prefix across Roblox updates.
- Pre-initialization shield: intercepts non-standard handshake probes (such as Antigravity's 'server/discover')
  and answers locally with JSON-RPC error -32601, ensuring StudioMCP.exe always receives 'initialize' as its very first packet.
- Wine IPC & stdio isolation: suppresses Wine debug noise (WINEDEBUG=-all) to guarantee 100% clean JSON-RPC streams.
"""

import os
import sys
import json
import glob
import signal
import argparse
import subprocess
import threading

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DEFAULT_PREFIX = os.path.join(REPO_ROOT, "sandbox", "prefix")


def find_studio_mcp(prefix_path: str) -> str:
    """Finds the most recent StudioMCP.exe inside official Roblox paths in the Wine prefix."""
    patterns = [
        # Standard user AppData Versions
        os.path.join(
            prefix_path,
            "drive_c",
            "users",
            "*",
            "AppData",
            "Local",
            "Roblox",
            "Versions",
            "version-*",
            "StudioMCP.exe",
        ),
        # Program Files (x86) Roblox
        os.path.join(
            prefix_path,
            "drive_c",
            "Program Files (x86)",
            "Roblox",
            "Versions",
            "version-*",
            "StudioMCP.exe",
        ),
        # Program Files Roblox
        os.path.join(
            prefix_path,
            "drive_c",
            "Program Files",
            "Roblox",
            "Versions",
            "version-*",
            "StudioMCP.exe",
        ),
    ]
    candidates = []
    for pattern in patterns:
        candidates.extend(glob.glob(pattern))

    if not candidates:
        return ""

    # Sort candidates by modification time descending (latest version first)
    candidates.sort(key=lambda p: os.path.getmtime(p), reverse=True)
    return candidates[0]


def pump_stream(src, dst):
    """Pumps text lines from source stream to destination stream."""
    try:
        for line in iter(src.readline, ""):
            if not line:
                break
            dst.write(line)
            dst.flush()
    except Exception:
        pass


def main():
    parser = argparse.ArgumentParser(
        description="Universal MCP Bridge for Roblox Studio running under Wine."
    )
    parser.add_argument(
        "--prefix",
        "-p",
        default=os.environ.get("WINEPREFIX", DEFAULT_PREFIX),
        help="Path to the Wine prefix containing Roblox Studio.",
    )
    parser.add_argument(
        "--exe",
        "-e",
        default=os.environ.get("ROBLOX_STUDIOMCP_EXE", ""),
        help="Direct path to StudioMCP.exe (optional, auto-detected if omitted).",
    )
    parser.add_argument(
        "--wine",
        default=os.environ.get("WINE", "wine"),
        help="Wine executable binary to use (default: wine).",
    )
    args, unknown = parser.parse_known_args()

    # UTF-8 stdio guarantee
    if hasattr(sys.stdin, "reconfigure"):
        sys.stdin.reconfigure(encoding="utf-8")
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8")

    prefix_path = os.path.abspath(args.prefix)
    studio_mcp_path = args.exe or find_studio_mcp(prefix_path)

    if not studio_mcp_path or not os.path.isfile(studio_mcp_path):
        sys.stderr.write(
            f"[RobloxMCPBridge] Error: Could not locate StudioMCP.exe in prefix '{prefix_path}'.\n"
        )
        sys.stderr.write(
            "[RobloxMCPBridge] Please ensure Roblox Studio is installed in this prefix.\n"
        )
        sys.stderr.flush()
        sys.exit(1)

    sys.stderr.write(
        f"[RobloxMCPBridge] Found StudioMCP at: {studio_mcp_path}\n"
    )
    sys.stderr.write(f"[RobloxMCPBridge] Using Wine prefix: {prefix_path}\n")
    sys.stderr.flush()

    # Prepare Wine environment for StudioMCP
    wine_env = os.environ.copy()
    wine_env["WINEPREFIX"] = prefix_path
    wine_env["WINEDEBUG"] = "-all"  # Silence wine debug output completely
    wine_env["WINEFSYNC"] = "1"
    wine_env["WINENTSYNC"] = "1"

    # Spawn Wine process running StudioMCP.exe
    proc = subprocess.Popen(
        [args.wine, studio_mcp_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=wine_env,
        text=True,
        encoding="utf-8",
        bufsize=1,
    )

    # Background threads for stdout and stderr streaming
    threading.Thread(target=pump_stream, args=(proc.stdout, sys.stdout), daemon=True).start()
    threading.Thread(target=pump_stream, args=(proc.stderr, sys.stderr), daemon=True).start()

    has_initialized = False

    def handle_signal(sig, frame):
        try:
            if proc.poll() is None:
                proc.terminate()
        except Exception:
            pass
        sys.exit(0)

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    try:
        for line in sys.stdin:
            line_str = line.strip()
            if not line_str:
                continue

            # State check: Pre-initialization guard
            if not has_initialized:
                try:
                    msg = json.loads(line_str)
                    if isinstance(msg, dict):
                        method = msg.get("method")
                        # If a client probes with 'server/discover' or any non-initialize method
                        # before standard initialization, answer locally with standard JSON-RPC Method Not Found
                        if method and method != "initialize":
                            req_id = msg.get("id")
                            resp = {
                                "jsonrpc": "2.0",
                                "id": req_id,
                                "error": {
                                    "code": -32601,
                                    "message": f"Method '{method}' not found or server not initialized",
                                },
                            }
                            sys.stdout.write(json.dumps(resp) + "\n")
                            sys.stdout.flush()
                            continue
                        elif method == "initialize":
                            has_initialized = True
                except Exception:
                    # Non-JSON or malformed packet, allow forwarding
                    pass

            if proc.poll() is not None:
                break

            # Forward verified packet to StudioMCP.exe
            proc.stdin.write(line_str + "\n")
            proc.stdin.flush()

    except (KeyboardInterrupt, BrokenPipeError):
        pass
    finally:
        try:
            if proc.stdin:
                proc.stdin.close()
        except Exception:
            pass
        try:
            proc.wait(timeout=3)
        except Exception:
            try:
                proc.terminate()
            except Exception:
                pass


if __name__ == "__main__":
    main()
