#!/usr/bin/env python3
"""
Universal Roblox Studio MCP Bridge for Linux / Wine
Compatible with Claude Desktop, Cursor, Codex, Antigravity, and any standard MCP client.

Features:
- Dynamically discovers the latest StudioMCP.exe inside the active Wine prefix across Roblox updates.
- Pre-initialization shield: intercepts non-standard handshake probes (such as Antigravity's 'server/discover')
  and answers locally with JSON-RPC error -32601, ensuring StudioMCP.exe always receives 'initialize' as its very first packet.
- Wine IPC & stdio isolation: suppresses Wine debug noise (WINEDEBUG=-all) to guarantee 100% clean JSON-RPC streams.
- Closes with Studio: once Roblox Studio has run in the prefix and then closed, stops StudioMCP.exe and exits,
  so idle bridges don't keep the wineserver alive (disable with --keep-alive or CUTLERY_MCP_EXIT_WITH_STUDIO=0).
"""

import os
import re
import sys
import json
import glob
import time
import signal
import argparse
import subprocess
import threading

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DEFAULT_PREFIX = os.path.join(REPO_ROOT, "sandbox", "prefix")

# RobloxStudioCutlery.exe is the themed copy Cutlery launches instead of RobloxStudioBeta.exe.
STUDIO_EXE = re.compile(r"^robloxstudio(beta|cutlery)\.exe$", re.IGNORECASE)
STUDIO_POLL_SECONDS = 2
# Studio briefly has no process when it restarts itself (updates, opening another place).
STUDIO_GONE_GRACE_SECONDS = 10


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


def studio_running(prefix_path: str) -> bool:
    """True if a Roblox Studio process is running in the given Wine prefix."""
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                argv0 = f.read().split(b"\0", 1)[0].decode("utf-8", "replace")
            # Wine shows either the Unix path or the Windows path of the exe.
            if not STUDIO_EXE.match(argv0.replace("\\", "/").rsplit("/", 1)[-1]):
                continue
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(
                    item.split(b"=", 1) for item in f.read().split(b"\0") if b"=" in item
                )
        except OSError:
            continue
        wine_prefix = env.get(b"WINEPREFIX")
        if wine_prefix is None or os.path.realpath(wine_prefix.decode("utf-8", "replace")) == prefix_path:
            return True
    return False


def watch_studio(prefix_path: str, on_closed):
    """Calls on_closed once Studio has been seen running and then stays gone for the grace period."""
    seen = False
    gone_since = None
    while True:
        if studio_running(prefix_path):
            seen = True
            gone_since = None
        elif seen:
            gone_since = gone_since or time.monotonic()
            if time.monotonic() - gone_since >= STUDIO_GONE_GRACE_SECONDS:
                on_closed()
                return
        time.sleep(STUDIO_POLL_SECONDS)


def stop_process(proc, timeout=3):
    """Closes stdin, then escalates to SIGTERM and SIGKILL until the process is gone."""
    try:
        if proc.stdin:
            proc.stdin.close()
    except Exception:
        pass
    for escalate in (None, proc.terminate, proc.kill):
        try:
            if escalate:
                escalate()
            proc.wait(timeout=timeout)
            return
        except Exception:
            pass


def env_flag(name: str, default: bool) -> bool:
    value = os.environ.get(name)
    if value is None:
        return default
    return value.strip().lower() not in ("0", "false", "no", "off", "")


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
    parser.add_argument(
        "--keep-alive",
        dest="exit_with_studio",
        action="store_false",
        default=env_flag("CUTLERY_MCP_EXIT_WITH_STUDIO", True),
        help="Keep running after Roblox Studio closes (default: exit once Studio closes).",
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
        stop_process(proc, timeout=1)
        sys.exit(0)

    def on_studio_closed():
        sys.stderr.write("[RobloxMCPBridge] Roblox Studio closed; shutting down the bridge.\n")
        sys.stderr.flush()
        stop_process(proc)
        # The main thread is blocked reading stdin, so leave without waiting for it.
        os._exit(0)

    if args.exit_with_studio:
        threading.Thread(
            target=watch_studio, args=(os.path.realpath(prefix_path), on_studio_closed), daemon=True
        ).start()

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
        stop_process(proc)


if __name__ == "__main__":
    main()
