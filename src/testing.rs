//! `spawn_test_host` — the harness the rest of the project tests through.
//!
//! One seam: the host boundary. Tests boot the host in-process with a temp socket path, a temp
//! database, and a controllable clock, then drive it by dialing the **real unix socket** and
//! writing the **same frames the hook writes**. Key events go in on the same channel the real input
//! thread uses (ADR-0009), so no test needs a terminal.
//!
//! Tests assert on what the developer could observe — the pane's contents and what persisted — and
//! never reach into the open-Trigger set or the Review state machine.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Duration, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use tempfile::TempDir;

use crate::clock::TestClock;
use crate::event::Event;
use crate::frame::{FrameType, TriggerFrame};
use crate::host::{DEFAULT_TRIGGER_EXPIRY_SECONDS, Host};
use crate::learning::Learning;
use crate::listener;
use crate::storage::{NewCard, Storage};

/// How long a `wait_for` will poll before failing the test. Generous: it bounds a genuine
/// deadlock, not a slow machine.
const WAIT_TIMEOUT: StdDuration = StdDuration::from_secs(5);

/// The card the harness seeds by default. Spine tests use its front as the stable "a card is up"
/// marker, and the front/back are distinct enough to prove the answer stays hidden until reveal.
pub const PLACEHOLDER_CARD_FRONT: &str = "What does FSRS stand for?";
pub const PLACEHOLDER_CARD_BACK: &str = "Free Spaced Repetition Scheduler";

/// A fixed, arbitrary instant. Tests advance from here explicitly; nothing depends on the real
/// clock.
pub fn test_epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()
}

pub struct TestHost {
    pub clock: Arc<TestClock>,
    socket_path: PathBuf,
    db_path: PathBuf,
    expiry: Duration,
    tx: Sender<Event>,
    pane: Arc<Mutex<String>>,
    /// Incremented once per draw, and the host draws once per event it processes.
    ///
    /// This is how a test knows an event has actually been *applied* rather than merely sent.
    draws: Arc<AtomicU64>,
    host_thread: Option<JoinHandle<()>>,
    _dir: TempDir,
}

/// Boot a host over a fresh temp database, a controllable clock, and an in-memory terminal, seeding
/// `cards` (front, back) into the deck first.
pub fn spawn_test_host() -> TestHost {
    fresh(
        Duration::seconds(DEFAULT_TRIGGER_EXPIRY_SECONDS),
        &[(PLACEHOLDER_CARD_FRONT, PLACEHOLDER_CARD_BACK)],
        None,
    )
}

pub fn spawn_test_host_with_expiry(expiry: Duration) -> TestHost {
    fresh(
        expiry,
        &[(PLACEHOLDER_CARD_FRONT, PLACEHOLDER_CARD_BACK)],
        None,
    )
}

/// Boot with a caller-supplied deck, for tests that need more than the default card.
pub fn spawn_test_host_with_cards(cards: &[(&str, &str)]) -> TestHost {
    fresh(
        Duration::seconds(DEFAULT_TRIGGER_EXPIRY_SECONDS),
        cards,
        None,
    )
}

/// Boot with a caller-supplied deck and daily new-card cap, for the scheduling tests.
pub fn spawn_test_host_with_cap_and_cards(cap: i64, cards: &[(&str, &str)]) -> TestHost {
    fresh(
        Duration::seconds(DEFAULT_TRIGGER_EXPIRY_SECONDS),
        cards,
        Some(cap),
    )
}

/// Boot over a brand-new temp data directory and database.
fn fresh(expiry: Duration, cards: &[(&str, &str)], cap: Option<i64>) -> TestHost {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("learnwhile.db");
    boot(dir, db_path, expiry, cards, cap)
}

fn boot(
    dir: TempDir,
    db_path: PathBuf,
    expiry: Duration,
    cards: &[(&str, &str)],
    cap: Option<i64>,
) -> TestHost {
    // A fresh socket name per boot: a restarted host cannot rebind the previous socket, since the
    // old listener thread lives on (the real binary would have exited and freed it). The database
    // is what persists across a restart, not the socket.
    static SOCKET_SEQ: AtomicU64 = AtomicU64::new(0);
    let socket_path = dir.path().join(format!(
        "learnwhile-{}.sock",
        SOCKET_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let clock = TestClock::new(test_epoch());

    let mut storage = Storage::open(&db_path).expect("open test db");
    if !cards.is_empty() {
        let new_cards: Vec<NewCard> = cards
            .iter()
            .map(|(front, back)| NewCard {
                front: (*front).to_string(),
                back: (*back).to_string(),
            })
            .collect();
        storage.seed_cards(&new_cards, test_epoch()).expect("seed");
    }
    if let Some(cap) = cap {
        storage
            .set_config("new_cards_per_day", &cap.to_string())
            .expect("set cap");
    }
    let learning = Learning::new(storage, clock.clone()).expect("learning");

    let (tx, rx) = channel();

    let listener = listener::bind(&socket_path).expect("bind test socket");
    let listener_tx = tx.clone();
    std::thread::spawn(move || listener::serve(listener, listener_tx));

    let pane = Arc::new(Mutex::new(String::new()));
    let observer_pane = Arc::clone(&pane);
    let draws = Arc::new(AtomicU64::new(0));
    let observer_draws = Arc::clone(&draws);

    let terminal = Terminal::new(TestBackend::new(60, 12)).expect("test terminal");
    let host = Host::new(terminal, clock.clone(), expiry, learning).with_draw_observer(Box::new(
        move |buffer: &Buffer| {
            *observer_pane.lock().expect("pane poisoned") = buffer_text(buffer);
            // Ordered after the pane write, so a test that sees the counter move sees the new
            // pane too.
            observer_draws.fetch_add(1, Ordering::Release);
        },
    ));

    let host_thread = std::thread::spawn(move || {
        host.run(rx).expect("host loop");
    });

    TestHost {
        clock,
        socket_path,
        db_path,
        expiry,
        tx,
        pane,
        draws,
        host_thread: Some(host_thread),
        _dir: dir,
    }
}

impl TestHost {
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// The database this host is reading and writing, for direct SQLite assertions.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Write a frame over the real socket, exactly as the hook does, and return once the host has
    /// applied it. Synchronous by design: an asynchronous send would make every clock-advancing
    /// test a race.
    pub fn send(&self, frame_type: FrameType, session: &str) {
        let frame = TriggerFrame::new(
            frame_type,
            crate::hook::ADAPTER_NAME,
            session,
            self.clock.now_for_frame(),
        );
        let before = self.draws.load(Ordering::Acquire);
        self.send_raw(&frame.to_line().expect("serialize frame"));
        self.await_draw_after(before, "a frame to be applied");
    }

    pub fn open(&self, session: &str) {
        self.send(FrameType::TriggerOpen, session);
    }

    pub fn close(&self, session: &str) {
        self.send(FrameType::TriggerClose, session);
    }

    /// Write arbitrary bytes down the socket — for the malformed-input cases.
    ///
    /// Deliberately not synchronous: a discarded line never reaches the loop, so there is no draw
    /// to wait for. Tests that use this assert on the host still working afterwards instead.
    pub fn send_raw(&self, raw: &str) {
        let mut stream = UnixStream::connect(&self.socket_path).expect("connect to test host");
        stream.write_all(raw.as_bytes()).expect("write frame");
        stream.flush().expect("flush frame");
    }

    /// Inject a keypress without waiting. Used for quit, where the loop exits without drawing.
    pub fn key(&self, code: KeyCode) {
        self.tx
            .send(Event::Key(KeyEvent::new_with_kind(
                code,
                KeyModifiers::NONE,
                KeyEventKind::Press,
            )))
            .expect("send key");
    }

    /// Inject a keypress and return once the host has drawn in response. Used for reveal and
    /// ratings, where the test then inspects the pane or the database and must not race the loop —
    /// a rating persists before the draw, so once this returns the row is committed.
    pub fn press(&self, code: KeyCode) {
        let before = self.draws.load(Ordering::Acquire);
        self.key(code);
        self.await_draw_after(before, "a key to be handled");
    }

    /// Fire the expiry sweep and return once it has run. Tests drive this directly rather than
    /// waiting on the real tick thread, so a sweep test is deterministic instead of a race
    /// against wall-clock.
    pub fn tick(&self) {
        let before = self.draws.load(Ordering::Acquire);
        self.tx.send(Event::Tick).expect("send tick");
        self.await_draw_after(before, "the sweep to run");
    }

    /// What the developer would see right now.
    pub fn pane(&self) -> String {
        self.pane.lock().expect("pane poisoned").clone()
    }

    /// Poll until the pane contains `needle`. Panics with the pane's contents on timeout, so a
    /// failure says what was on screen instead of just "timed out".
    pub fn wait_for(&self, needle: &str) {
        self.wait_until(
            |pane| pane.contains(needle),
            &format!("pane to contain {needle:?}"),
        );
    }

    /// Poll until the pane no longer contains `needle`.
    pub fn wait_for_absent(&self, needle: &str) {
        self.wait_until(
            |pane| !pane.contains(needle),
            &format!("pane to stop containing {needle:?}"),
        );
    }

    /// Block until the host has drawn at least once since `before`.
    fn await_draw_after(&self, before: u64, description: &str) {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while self.draws.load(Ordering::Acquire) <= before {
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for {description}. Pane was:\n{}",
                    self.pane()
                );
            }
            sleep(StdDuration::from_millis(2));
        }
    }

    fn wait_until(&self, predicate: impl Fn(&str) -> bool, description: &str) {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        loop {
            let pane = self.pane();
            if predicate(&pane) {
                return;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for {description}. Pane was:\n{pane}");
            }
            sleep(StdDuration::from_millis(5));
        }
    }

    /// Quit the host and reboot a fresh one against the **same** database and socket, preserving
    /// everything that persisted. This is how the restart-survival test reopens the temp database.
    pub fn restart(mut self) -> TestHost {
        self.key(KeyCode::Char('q'));
        if let Some(handle) = self.host_thread.take() {
            handle.join().expect("host thread");
        }
        // Reopen the same database in the same temp directory. Capture what boot needs before
        // moving the TempDir out (which keeps it, and the database inside it, alive). No reseeding
        // and no cap override: the cards, history, and config already persisted are what matter.
        let db_path = self.db_path.clone();
        let expiry = self.expiry;
        boot(self._dir, db_path, expiry, &[], None)
    }

    /// Quit the host and wait for the loop to finish.
    pub fn shutdown(mut self) {
        self.key(KeyCode::Char('q'));
        if let Some(handle) = self.host_thread.take() {
            handle.join().expect("host thread");
        }
    }
}

/// Flatten a rendered buffer into text, one line per row.
pub fn buffer_text(buffer: &Buffer) -> String {
    let width = buffer.area.width as usize;
    if width == 0 {
        return String::new();
    }
    buffer
        .content
        .chunks(width)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl TestClock {
    /// The `at` a frame carries. The host does not trust it for expiry, but it must be a real
    /// timestamp for the frame to parse.
    fn now_for_frame(&self) -> DateTime<Utc> {
        use crate::clock::Clock;
        self.now()
    }
}
