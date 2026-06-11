//! gdtf-sync — download the GDTF Share fixture catalogue to a local directory.
//!
//! Usage:
//!   cargo run --bin gdtf-sync -- --user EMAIL --pass PASSWORD
//!   cargo run --bin gdtf-sync -- --user EMAIL --pass PASSWORD --out assets/gdtf
//!   cargo run --bin gdtf-sync -- --user EMAIL --pass PASSWORD --force
//!
//! On first run every fixture is downloaded. On subsequent runs only fixtures
//! added or modified since the last sync are fetched. A `.gdtf_sync.json` state
//! file in the output directory tracks what has been downloaded.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://gdtf-share.com/apis/public";
const DEFAULT_OUT: &str = "assets/gdtf";
const DEFAULT_DELAY_MS: u64 = 150;

// ─── CLI args ─────────────────────────────────────────────────────────────────

struct Args {
    user: String,
    pass: String,
    out: PathBuf,
    delay_ms: u64,
    /// Re-download even if rid is already in state.
    force: bool,
    /// Only list what would be downloaded, don't fetch.
    dry_run: bool,
}

fn parse_args() -> Args {
    let mut user = String::new();
    let mut pass = String::new();
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut delay_ms = DEFAULT_DELAY_MS;
    let mut force = false;
    let mut dry_run = false;

    let mut iter = std::env::args().skip(1).peekable();
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--user" | "-u" => user = iter.next().expect("--user needs a value"),
            "--pass" | "-p" => pass = iter.next().expect("--pass needs a value"),
            "--out"  | "-o" => out  = PathBuf::from(iter.next().expect("--out needs a path")),
            "--delay"       => delay_ms = iter.next().expect("--delay needs ms")
                                          .parse().expect("--delay must be a number"),
            "--force"       => force   = true,
            "--dry-run"     => dry_run = true,
            "--help" | "-h" => { print_help(); std::process::exit(0); }
            other => {
                eprintln!("Unknown flag: {other}");
                print_help();
                std::process::exit(1);
            }
        }
    }

    if user.is_empty() || pass.is_empty() {
        eprintln!("Error: --user and --pass are required.\n");
        print_help();
        std::process::exit(1);
    }

    Args { user, pass, out, delay_ms, force, dry_run }
}

fn print_help() {
    eprintln!(concat!(
        "gdtf-sync — sync the GDTF Share fixture catalogue to a local folder\n\n",
        "USAGE:\n",
        "  cargo run --bin gdtf-sync -- --user EMAIL --pass PASSWORD [OPTIONS]\n\n",
        "OPTIONS:\n",
        "  --user, -u EMAIL     GDTF Share account e-mail (required)\n",
        "  --pass, -p PASSWORD  GDTF Share password (required)\n",
        "  --out,  -o DIR       Output directory (default: assets/gdtf)\n",
        "  --delay    MS        Milliseconds between downloads (default: 150)\n",
        "  --force              Re-download files already in the local cache\n",
        "  --dry-run            List what would be downloaded without fetching\n",
    ));
}

// ─── API types ────────────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct LoginResponse {
    result: bool,
    notice: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ListResponse {
    result: bool,
    list: Option<Vec<FixtureEntry>>,
    error: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct FixtureEntry {
    rid: u64,
    fixture: String,
    manufacturer: String,
    revision: String,
    #[serde(rename = "lastModified")]
    last_modified: Option<i64>,
    filesize: Option<i64>,
}

// ─── State file ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct SyncState {
    /// Unix timestamp of the last completed sync.
    last_sync: i64,
    /// Maps rid → filename for every successfully downloaded file.
    downloaded: HashMap<u64, String>,
}

fn state_path(out: &Path) -> PathBuf {
    out.join(".gdtf_sync.json")
}

fn load_state(out: &Path) -> SyncState {
    fs::read(state_path(out))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_state(out: &Path, state: &SyncState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(state_path(out), json);
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Replace anything that isn't alphanumeric, dash, or dot with underscore.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect()
}

fn fmt_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();

    fs::create_dir_all(&args.out)
        .unwrap_or_else(|e| { eprintln!("Cannot create output dir: {e}"); std::process::exit(1); });

    // ureq Agent maintains a cookie jar automatically — the session cookie from
    // login.php is re-sent on every subsequent request.
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(60))
        .build();

    // ── Login ─────────────────────────────────────────────────────────────────
    eprint!("Logging in as {} … ", args.user);
    std::io::stderr().flush().ok();

    let login: LoginResponse = agent
        .post(&format!("{API_BASE}/login.php"))
        .send_json(serde_json::json!({ "user": args.user, "password": args.pass }))
        .unwrap_or_else(|e| { eprintln!("\nHTTP error during login: {e}"); std::process::exit(1); })
        .into_json()
        .unwrap_or_else(|e| { eprintln!("\nCould not parse login response: {e}"); std::process::exit(1); });

    if !login.result {
        eprintln!("\nLogin failed: {}", login.error.as_deref().unwrap_or("unknown error"));
        std::process::exit(1);
    }
    eprintln!("{}", login.notice.as_deref().unwrap_or("OK"));

    // ── Fetch fixture list ────────────────────────────────────────────────────
    eprint!("Fetching fixture catalogue … ");
    std::io::stderr().flush().ok();

    let list_resp: ListResponse = agent
        .get(&format!("{API_BASE}/getList.php"))
        .call()
        .unwrap_or_else(|e| { eprintln!("\nHTTP error fetching list: {e}"); std::process::exit(1); })
        .into_json()
        .unwrap_or_else(|e| { eprintln!("\nCould not parse list response: {e}"); std::process::exit(1); });

    if !list_resp.result {
        eprintln!("\ngetList failed: {}", list_resp.error.as_deref().unwrap_or("unknown error"));
        std::process::exit(1);
    }

    let all_fixtures = list_resp.list.unwrap_or_default();
    eprintln!("{} fixtures in catalogue", all_fixtures.len());

    // ── Filter to what needs downloading ─────────────────────────────────────
    let mut state = load_state(&args.out);

    let to_download: Vec<&FixtureEntry> = all_fixtures.iter().filter(|f| {
        if args.force {
            return true;
        }
        if state.downloaded.contains_key(&f.rid) {
            return false;
        }
        // New fixture, or modified since our last sync
        f.last_modified.map_or(true, |t| t > state.last_sync)
    }).collect();

    let cached = state.downloaded.len();
    let total = to_download.len();

    if total == 0 {
        eprintln!("Already up to date — {cached} fixtures in local cache.");
        return;
    }
    eprintln!("{total} to download, {cached} already cached.\n");

    if args.dry_run {
        for f in &to_download {
            println!("{} — {} (rid {})", f.manufacturer, f.fixture, f.rid);
        }
        eprintln!("\n--dry-run: no files written.");
        return;
    }

    // ── Download loop ─────────────────────────────────────────────────────────
    let mut ok: usize = 0;
    let mut failed: usize = 0;

    for (i, fixture) in to_download.iter().enumerate() {
        let filename = format!(
            "{}_{}_{}.gdtf",
            sanitize(&fixture.manufacturer),
            sanitize(&fixture.fixture),
            fixture.rid,
        );
        let out_path = args.out.join(&filename);

        let size_hint = fixture.filesize
            .map(fmt_size)
            .unwrap_or_default();

        eprint!("[{:>4}/{total}] {}/{} rev:{} ({size_hint}) … ",
            i + 1,
            fixture.manufacturer,
            fixture.fixture,
            fixture.revision,
        );
        std::io::stderr().flush().ok();

        match agent
            .get(&format!("{API_BASE}/downloadFile.php?rid={}", fixture.rid))
            .call()
        {
            Ok(resp) => {
                let mut bytes = Vec::new();
                if let Err(e) = resp.into_reader().read_to_end(&mut bytes) {
                    eprintln!("read error: {e}");
                    failed += 1;
                } else if bytes.len() < 4 {
                    eprintln!("SKIP (empty response)");
                    failed += 1;
                } else {
                    fs::write(&out_path, &bytes)
                        .unwrap_or_else(|e| { eprintln!("write error: {e}"); });
                    state.downloaded.insert(fixture.rid, filename);
                    eprintln!("OK ({} B)", bytes.len());
                    ok += 1;
                }
            }
            Err(e) => {
                eprintln!("FAIL — {e}");
                failed += 1;
            }
        }

        // Checkpoint every 100 files so a crash doesn't lose all progress.
        if ok % 100 == 0 && ok > 0 {
            save_state(&args.out, &state);
        }

        if i + 1 < total {
            thread::sleep(Duration::from_millis(args.delay_ms));
        }
    }

    // ── Finalise ──────────────────────────────────────────────────────────────
    state.last_sync = now_unix();
    save_state(&args.out, &state);

    eprintln!("\nDone.  Downloaded: {ok}  Failed: {failed}  Total cached: {}",
        state.downloaded.len());

    if failed > 0 {
        std::process::exit(1);
    }
}
