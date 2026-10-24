# Discord Rich Presence

How Cutlery shows Roblox Studio activity on Discord. Code: `src/discord.rs`.

## Flow

1. `cutlery run` spawns Studio and, when `[discord] enabled = true`, a worker thread.
2. The worker tails the newest `*_Studio_*.log` written since launch in
   `<prefix>/drive_c/users/<user>/AppData/Local/Roblox/logs/` every 2 seconds.
3. Log lines drive a small state machine (start page → building/scripting → playtesting).
4. The place ID is turned into the experience name with Roblox's public APIs
   (`apis.roblox.com/universes/v1/places/{id}/universe`, then `games.roblox.com/v1/games`).
   Skipped in privacy mode.
5. The activity is sent to Discord over its IPC socket. It is only resent when it changes,
   at most once every 4 seconds (Discord allows 5 updates per 20 seconds).
6. When Studio exits, the presence is cleared.

## Log markers (Studio 0.744)

| Line contains | Meaning |
| --- | --- |
| `open place (identifier = <placeId>)` | A cloud place was opened |
| `Setting up for DataModel Edit` | Edit DataModel ready: a place or local file is open, or a playtest ended |
| `RobloxIDEDoc::activate` | A place tab got focus → Building |
| `RobloxScriptDoc::activate` | A script tab got focus → Scripting |
| `initiatePlaySolo` / `Setting up for DataModel Play…` | Playtest started |
| `becomes ClosingPlace` | The place was closed → start page |
| `exit LifecycleManager UserSession scope` | Studio is quitting → presence cleared |

Roblox can change these lines in any Studio release; `parse_line` has tests with real samples.

## IPC sockets

Searched in `$XDG_RUNTIME_DIR`, `/run/user/<uid>` and `/tmp`, in the subfolders `""`,
`app/com.discordapp.Discord` (Flatpak), `snap.discord` and `.flatpak/dev.vencord.Vesktop/xdg-run`,
for `discord-ipc-0` to `discord-ipc-9`. Frames are `op: u32 LE`, `length: u32 LE`, then JSON.

## Using your own Discord application

1. Create an application in the [Discord Developer Portal](https://discord.com/developers/applications).
   Its name is what Discord shows as "Playing …".
2. Under **Rich Presence → Art Assets**, upload images with these keys:
   `cutlery` (large image), `edit`, `script` and `play` (small icons).
3. Copy the **Application ID** into Universal MCP → Discord Rich Presence, or
   `application_id` under `[discord]` in `~/.config/cutlery/config.toml`.
