//! Discord Rich Presence for Roblox Studio.
//!
//! Follows the newest Studio log in the prefix, turns its lifecycle lines into a small
//! state machine and publishes it to the Discord client over its local IPC socket.
//! Everything here is best effort: no Discord, no log or no network just means no presence.

use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::config::DiscordConfig;

/// Cutlery's own Discord application, used when the config has no custom ID.
pub const DEFAULT_APPLICATION_ID: &str = "1563574202945241220";

const REPO_URL: &str = "https://github.com/denkalli/cutlery";
const TICK: Duration = Duration::from_secs(2);
const RECONNECT_DELAY: Duration = Duration::from_secs(15);
/// Discord allows 5 activity updates per 20 seconds.
const MIN_UPDATE_GAP: Duration = Duration::from_secs(4);

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

/// Where Discord, its Flatpak/Snap builds and Vesktop put `discord-ipc-N`.
const SOCKET_SUBDIRS: [&str; 4] = ["", "app/com.discordapp.Discord", "snap.discord", ".flatpak/dev.vencord.Vesktop/xdg-run"];

fn socket_candidates() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        bases.push(PathBuf::from(dir));
    }
    // /proc/self is owned by the process' user, which avoids pulling in libc for getuid().
    if let Ok(meta) = fs::metadata("/proc/self") {
        let run_user = PathBuf::from(format!("/run/user/{}", meta.uid()));
        if !bases.contains(&run_user) {
            bases.push(run_user);
        }
    }
    bases.push(PathBuf::from("/tmp"));

    let mut candidates = Vec::new();
    for base in &bases {
        for sub in SOCKET_SUBDIRS {
            for i in 0..10 {
                candidates.push(base.join(sub).join(format!("discord-ipc-{}", i)));
            }
        }
    }
    candidates
}

/// First Discord IPC socket that exists, for diagnostics.
pub fn find_socket() -> Option<PathBuf> {
    socket_candidates().into_iter().find(|p| p.exists())
}

fn encode_frame(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    frame
}

fn read_frame(reader: &mut impl Read) -> io::Result<(u32, Value)> {
    let mut header = [0u8; 8];
    reader.read_exact(&mut header)?;
    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if len > 1 << 20 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "oversized IPC frame"));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let payload = serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok((op, payload))
}

fn ipc_error(payload: &Value) -> io::Error {
    let message = payload["message"]
        .as_str()
        .or_else(|| payload["data"]["message"].as_str())
        .unwrap_or("unknown error");
    io::Error::other(format!("Discord refused the request: {}", message))
}

struct Ipc {
    stream: UnixStream,
    nonce: u64,
}

impl Ipc {
    fn connect(client_id: &str) -> io::Result<Self> {
        let mut last_err = io::Error::new(io::ErrorKind::NotFound, "Discord is not running");
        for path in socket_candidates() {
            if !path.exists() {
                continue;
            }
            match Self::handshake(&path, client_id) {
                Ok(ipc) => return Ok(ipc),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    fn handshake(path: &Path, client_id: &str) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut ipc = Self { stream, nonce: 0 };
        ipc.stream.write_all(&encode_frame(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id })))?;
        match read_frame(&mut ipc.stream)? {
            (OP_FRAME, payload) if payload["evt"] == "READY" => Ok(ipc),
            (_, payload) => Err(ipc_error(&payload)),
        }
    }

    /// Sets the activity, or clears it with `None`.
    fn set_activity(&mut self, activity: Option<&Value>) -> io::Result<()> {
        self.nonce += 1;
        let request = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": format!("cutlery-{}-{}", std::process::id(), self.nonce),
        });
        self.stream.write_all(&encode_frame(OP_FRAME, &request))?;
        match read_frame(&mut self.stream)? {
            (OP_CLOSE, payload) => Err(ipc_error(&payload)),
            (_, payload) if payload["evt"] == "ERROR" => Err(ipc_error(&payload)),
            _ => Ok(()),
        }
    }
}

/// A Studio log line that changes what the presence shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    PlaceOpened(u64),
    /// The Edit DataModel came up: a place (or local file) was opened, or a playtest ended.
    Editing,
    /// A place tab (viewport) got focus.
    Building,
    /// A script tab got focus.
    Scripting,
    Playtesting,
    PlaceClosed,
    Exiting,
}

fn parse_line(line: &str) -> Option<Event> {
    if let Some(rest) = line.split("open place (identifier = ").nth(1) {
        return rest.split(')').next()?.trim().parse().ok().map(Event::PlaceOpened);
    }
    if line.contains("Setting up for DataModel Edit") {
        Some(Event::Editing)
    } else if line.contains("Setting up for DataModel Play") || line.contains("initiatePlaySolo") {
        Some(Event::Playtesting)
    } else if line.contains("RobloxScriptDoc::activate") {
        Some(Event::Scripting)
    } else if line.contains("RobloxIDEDoc::activate") {
        Some(Event::Building)
    } else if line.contains("becomes ClosingPlace") {
        Some(Event::PlaceClosed)
    } else if line.contains("exit LifecycleManager UserSession scope") {
        Some(Event::Exiting)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    StartPage,
    Building,
    Scripting,
    Playtesting,
    Exiting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StudioState {
    place_id: Option<u64>,
    mode: Mode,
    /// Unix seconds when the current place (or the start page) was entered.
    opened_at: u64,
    /// Unix seconds when the current playtest started.
    playtest_at: u64,
}

impl StudioState {
    fn new(now: u64) -> Self {
        Self { place_id: None, mode: Mode::StartPage, opened_at: now, playtest_at: now }
    }

    fn apply(&mut self, event: Event, now: u64) {
        match event {
            Event::PlaceOpened(id) => {
                self.place_id = Some(id);
                self.mode = Mode::Building;
                self.opened_at = now;
            }
            Event::Editing => match self.mode {
                Mode::Playtesting => self.mode = Mode::Building,
                // Local files have no "open place" line, so the Edit DataModel opens them.
                Mode::StartPage => {
                    self.mode = Mode::Building;
                    self.opened_at = now;
                }
                _ => {}
            },
            Event::Building | Event::Scripting if matches!(self.mode, Mode::Building | Mode::Scripting) => {
                self.mode = if event == Event::Scripting { Mode::Scripting } else { Mode::Building };
            }
            Event::Building | Event::Scripting => {}
            Event::Playtesting => {
                if self.mode != Mode::Playtesting {
                    self.mode = Mode::Playtesting;
                    self.playtest_at = now;
                }
            }
            Event::PlaceClosed => {
                self.place_id = None;
                self.mode = Mode::StartPage;
                self.opened_at = now;
            }
            Event::Exiting => self.mode = Mode::Exiting,
        }
    }
}

/// Discord limits the text fields to 128 characters.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut clipped: String = text.chars().take(max - 1).collect();
        clipped.push('…');
        clipped
    }
}

/// The activity for `state`, or `None` when the presence should be cleared.
fn build_activity(state: &StudioState, place_name: Option<&str>, cfg: &DiscordConfig) -> Option<Value> {
    let name = place_name.filter(|_| !cfg.privacy_mode).map(|n| clip(n, 100));
    let (details, status, small) = match state.mode {
        Mode::Exiting => return None,
        Mode::StartPage => ("In Roblox Studio".to_string(), "Start page", None),
        Mode::Playtesting => (
            name.as_ref().map_or("Testing a place".to_string(), |n| format!("Testing {}", n)),
            "Playtesting",
            Some(("play", "Playtesting")),
        ),
        Mode::Building | Mode::Scripting => {
            let details = name.as_ref().map_or("Editing a place".to_string(), |n| format!("Editing {}", n));
            if state.mode == Mode::Scripting {
                (details, "Scripting", Some(("script", "Scripting")))
            } else {
                (details, "Building", Some(("edit", "Building")))
            }
        }
    };

    let mut assets = json!({
        "large_image": "cutlery",
        "large_text": "Cutlery · Roblox Studio on Linux",
    });
    if let Some((image, text)) = small {
        assets["small_image"] = json!(image);
        assets["small_text"] = json!(text);
    }

    let mut activity = json!({ "details": details, "state": status, "assets": assets });
    if cfg.show_elapsed {
        let start = if state.mode == Mode::Playtesting { state.playtest_at } else { state.opened_at };
        activity["timestamps"] = json!({ "start": start });
    }
    if cfg.show_buttons {
        let mut buttons = Vec::new();
        if let (Some(_), Some(id)) = (&name, state.place_id) {
            buttons.push(json!({ "label": "View on Roblox", "url": format!("https://www.roblox.com/games/{}", id) }));
        }
        buttons.push(json!({ "label": "Get Cutlery", "url": REPO_URL }));
        activity["buttons"] = json!(buttons);
    }
    Some(activity)
}

/// Experience name for a place ID, from Roblox's public (unauthenticated) APIs.
fn resolve_place_name(place_id: u64) -> Option<String> {
    let universe: Value = serde_json::from_str(
        &crate::update::curl_get(&format!("https://apis.roblox.com/universes/v1/places/{}/universe", place_id)).ok()?,
    )
    .ok()?;
    let universe_id = universe["universeId"].as_u64()?;
    let games: Value = serde_json::from_str(
        &crate::update::curl_get(&format!("https://games.roblox.com/v1/games?universeIds={}", universe_id)).ok()?,
    )
    .ok()?;
    games["data"][0]["name"]
        .as_str()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
}

/// Tails the newest `*_Studio_*.log` written since the launch.
struct LogWatcher {
    dir: PathBuf,
    since: SystemTime,
    current: Option<(PathBuf, u64)>,
    partial: String,
}

impl LogWatcher {
    fn new(dir: PathBuf, since: SystemTime) -> Self {
        Self { dir, since, current: None, partial: String::new() }
    }

    fn newest_log(&self) -> Option<PathBuf> {
        fs::read_dir(&self.dir)
            .ok()?
            .flatten()
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.contains("_Studio_") && name.ends_with(".log")
            })
            .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
            .filter(|(modified, _)| *modified >= self.since)
            .max_by_key(|(modified, _)| *modified)
            .map(|(_, path)| path)
    }

    fn poll(&mut self) -> Vec<Event> {
        if let Some(newest) = self.newest_log() {
            if self.current.as_ref().is_none_or(|(path, _)| *path != newest) {
                self.current = Some((newest, 0));
                self.partial.clear();
            }
        }
        let Some((path, offset)) = &mut self.current else { return Vec::new() };
        let Ok(mut file) = fs::File::open(&*path) else { return Vec::new() };
        if file.metadata().map_or(false, |m| m.len() < *offset) {
            *offset = 0;
            self.partial.clear();
        }
        let mut bytes = Vec::new();
        if file.seek(SeekFrom::Start(*offset)).is_err() || file.read_to_end(&mut bytes).is_err() {
            return Vec::new();
        }
        *offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));

        let complete = match self.partial.rfind('\n') {
            Some(end) => self.partial.drain(..=end).collect::<String>(),
            None => return Vec::new(),
        };
        complete.lines().filter_map(parse_line).collect()
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Running presence worker; dropping it without `shutdown` leaves the thread to exit with the process.
pub struct Handle {
    stop: Sender<()>,
    thread: JoinHandle<()>,
}

impl Handle {
    /// Clears the presence and waits for the worker to finish.
    pub fn shutdown(self) {
        let _ = self.stop.send(());
        let _ = self.thread.join();
    }
}

/// Starts the presence worker for a Studio session whose logs go to `logs_dir`.
pub fn spawn(cfg: DiscordConfig, logs_dir: PathBuf) -> Handle {
    let client_id = match cfg.application_id.trim() {
        "" => DEFAULT_APPLICATION_ID.to_string(),
        custom => custom.to_string(),
    };
    // Studio's log is created after launch; allow for clock skew between the prefix and us.
    let since = SystemTime::now() - Duration::from_secs(5);
    let (stop, stop_rx) = mpsc::channel();
    let thread = thread::spawn(move || run(cfg, client_id, LogWatcher::new(logs_dir, since), stop_rx));
    Handle { stop, thread }
}

fn run(cfg: DiscordConfig, client_id: String, mut logs: LogWatcher, stop: Receiver<()>) {
    let mut state = StudioState::new(unix_now());
    let mut names: std::collections::HashMap<u64, Option<String>> = Default::default();
    let (name_tx, name_rx) = mpsc::channel::<(u64, Option<String>)>();

    let mut ipc: Option<Ipc> = None;
    let mut next_connect = Instant::now();
    let mut sent: Option<Option<Value>> = None;
    let mut last_send = Instant::now() - MIN_UPDATE_GAP;

    loop {
        for event in logs.poll() {
            state.apply(event, unix_now());
        }
        while let Ok((id, name)) = name_rx.try_recv() {
            names.insert(id, name);
        }
        if let Some(id) = state.place_id.filter(|id| !cfg.privacy_mode && !names.contains_key(id)) {
            names.insert(id, None);
            let tx = name_tx.clone();
            thread::spawn(move || {
                let _ = tx.send((id, resolve_place_name(id)));
            });
        }

        if ipc.is_none() && Instant::now() >= next_connect {
            match Ipc::connect(&client_id) {
                Ok(conn) => {
                    println!("[Cutlery] [Discord] Rich Presence connected.");
                    ipc = Some(conn);
                    sent = None;
                }
                Err(_) => next_connect = Instant::now() + RECONNECT_DELAY,
            }
        }

        let name = state.place_id.and_then(|id| names.get(&id).cloned().flatten());
        let activity = build_activity(&state, name.as_deref(), &cfg);
        if let Some(conn) = ipc.as_mut() {
            if sent.as_ref() != Some(&activity) && last_send.elapsed() >= MIN_UPDATE_GAP {
                last_send = Instant::now();
                match conn.set_activity(activity.as_ref()) {
                    Ok(()) => sent = Some(activity),
                    Err(e) => {
                        eprintln!("[Cutlery] [Discord] {}", e);
                        ipc = None;
                        next_connect = Instant::now() + RECONNECT_DELAY;
                    }
                }
            }
        }

        match stop.recv_timeout(TICK) {
            Err(RecvTimeoutError::Timeout) => {}
            _ => break,
        }
    }

    if let Some(mut conn) = ipc {
        let _ = conn.set_activity(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lines copied from a Studio 0.744 log.
    const OPEN: &str = "2026-10-24T06:41:11.905Z,13.905000,0024,6,Info [FLog::StudioKeyEvents] open place (identifier = 95206881) [start]";
    const EDIT: &str = "2026-10-24T06:41:13.536Z,15.536000,0024,6,Info [FLog::RenderSchedulerState] Setting up for DataModel Edit with fallback Standalone";
    const STANDALONE: &str = "2026-10-24T06:41:34.843Z,36.843000,0024,6,Info [FLog::RenderSchedulerState] Setting up for DataModel Standalone with fallback nullptr";
    const SCRIPT: &str = "2026-10-24T06:41:25.303Z,27.303000,0024,6 [FLog::Output] Info: RobloxScriptDoc::activate - start";
    const PLACE_TAB: &str = "2026-10-24T06:41:26.001Z,28.001000,0024,6,Info [FLog::RobloxIDEDoc] RobloxIDEDoc::activate - start";
    const PLAY: &str = "2026-10-24T05:42:20.980Z,192.980000,0024,6,Info [FLog::PlaySoloManager] initiatePlaySolo: PlaceStateTransitionStatus becomes StartingPlayTest";
    const PLAY_CLIENT: &str = "2026-10-24T05:42:21.500Z,193.500000,0024,6,Info [FLog::RenderSchedulerState] Setting up for DataModel PlayClient with fallback Standalone";
    const CLOSING: &str = "2026-10-24T06:41:34.300Z,36.300000,0024,6,Info [FLog::PlaceStateTransition] PlaceStateTransitionStatus becomes ClosingPlace";
    const EXIT: &str = "2026-10-24T06:41:35.005Z,37.005000,0024,6,Info [FLog::StudioKeyEvents] exit LifecycleManager UserSession scope (standalone shutdown)";

    #[test]
    fn test_parse_line() {
        assert_eq!(parse_line(OPEN), Some(Event::PlaceOpened(95206881)));
        assert_eq!(parse_line(EDIT), Some(Event::Editing));
        assert_eq!(parse_line(STANDALONE), None);
        assert_eq!(parse_line(SCRIPT), Some(Event::Scripting));
        assert_eq!(parse_line(PLACE_TAB), Some(Event::Building));
        assert_eq!(parse_line(PLAY), Some(Event::Playtesting));
        assert_eq!(parse_line(PLAY_CLIENT), Some(Event::Playtesting));
        assert_eq!(parse_line(CLOSING), Some(Event::PlaceClosed));
        assert_eq!(parse_line(EXIT), Some(Event::Exiting));
        assert_eq!(parse_line("[FLog::StudioKeyEvents] open place (identifier = abc) [start]"), None);
    }

    #[test]
    fn test_state_machine() {
        let mut s = StudioState::new(100);
        s.apply(Event::PlaceOpened(42), 110);
        s.apply(Event::Editing, 111);
        assert_eq!((s.place_id, s.mode, s.opened_at), (Some(42), Mode::Building, 110));
        s.apply(Event::Scripting, 120);
        assert_eq!(s.mode, Mode::Scripting);
        s.apply(Event::Playtesting, 130);
        s.apply(Event::Playtesting, 131);
        s.apply(Event::Scripting, 132);
        assert_eq!((s.mode, s.playtest_at), (Mode::Playtesting, 130));
        s.apply(Event::Editing, 140);
        assert_eq!((s.mode, s.opened_at), (Mode::Building, 110));
        s.apply(Event::PlaceClosed, 150);
        assert_eq!((s.place_id, s.mode, s.opened_at), (None, Mode::StartPage, 150));
        // A local file shows up only as a new Edit DataModel.
        s.apply(Event::Editing, 160);
        assert_eq!((s.place_id, s.mode, s.opened_at), (None, Mode::Building, 160));
        s.apply(Event::Exiting, 170);
        assert_eq!(s.mode, Mode::Exiting);
    }

    #[test]
    fn test_activity_with_place_name() {
        let mut s = StudioState::new(100);
        s.apply(Event::PlaceOpened(95206881), 110);
        let a = build_activity(&s, Some("Baseplate"), &DiscordConfig::default()).unwrap();
        assert_eq!(a["details"], "Editing Baseplate");
        assert_eq!(a["state"], "Building");
        assert_eq!(a["assets"]["small_image"], "edit");
        assert_eq!(a["timestamps"]["start"], 110);
        assert_eq!(a["buttons"][0]["url"], "https://www.roblox.com/games/95206881");
        assert_eq!(a["buttons"][1]["url"], REPO_URL);

        s.apply(Event::Playtesting, 120);
        let a = build_activity(&s, Some("Baseplate"), &DiscordConfig::default()).unwrap();
        assert_eq!(a["details"], "Testing Baseplate");
        assert_eq!(a["timestamps"]["start"], 120);
    }

    #[test]
    fn test_activity_privacy_and_options() {
        let mut s = StudioState::new(100);
        s.apply(Event::PlaceOpened(95206881), 110);
        let cfg = DiscordConfig { privacy_mode: true, show_elapsed: false, ..Default::default() };
        let a = build_activity(&s, Some("Secret Project"), &cfg).unwrap();
        assert_eq!(a["details"], "Editing a place");
        assert!(a.get("timestamps").is_none());
        assert_eq!(a["buttons"].as_array().unwrap().len(), 1);

        let cfg = DiscordConfig { show_buttons: false, ..Default::default() };
        assert!(build_activity(&s, Some("Baseplate"), &cfg).unwrap().get("buttons").is_none());

        s.apply(Event::Exiting, 120);
        assert_eq!(build_activity(&s, None, &DiscordConfig::default()), None);
    }

    #[test]
    fn test_frame_roundtrip() {
        let payload = json!({ "v": 1, "client_id": "123" });
        let frame = encode_frame(OP_HANDSHAKE, &payload);
        assert_eq!(&frame[..4], &0u32.to_le_bytes());
        assert_eq!(read_frame(&mut frame.as_slice()).unwrap(), (OP_HANDSHAKE, payload));
    }

    #[test]
    fn test_clip() {
        assert_eq!(clip("short", 10), "short");
        assert_eq!(clip("abcdefghijk", 5), "abcd…");
    }

    #[test]
    fn test_log_watcher_tails_new_lines() {
        let dir = std::env::temp_dir().join(format!("cutlery-discord-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let log = dir.join("0.744.0_20261024T064105Z_Studio_32A7E_last.log");
        fs::write(&log, format!("{}\n{}", OPEN, &EDIT[..20])).unwrap();

        let mut watcher = LogWatcher::new(dir.clone(), UNIX_EPOCH);
        assert_eq!(watcher.poll(), vec![Event::PlaceOpened(95206881)]);
        let mut file = fs::OpenOptions::new().append(true).open(&log).unwrap();
        writeln!(file, "{}", &EDIT[20..]).unwrap();
        writeln!(file, "{}", PLAY).unwrap();
        assert_eq!(watcher.poll(), vec![Event::Editing, Event::Playtesting]);
        assert_eq!(watcher.poll(), vec![]);
        let _ = fs::remove_dir_all(&dir);
    }
}
