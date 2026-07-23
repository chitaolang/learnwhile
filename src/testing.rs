//! `spawn_test_host` — the harness the rest of the project tests through.
//!
//! One seam: the host boundary. Tests boot the host in-process with a temp socket path and a
//! controllable clock, then drive it by dialing the **real unix socket** and writing the **same
//! frames the hook writes**. Key events go in on the same channel the real input thread uses
//! (ADR-0009), so no test needs a terminal.
//!
//! Tests assert on what the developer could observe — the pane's contents — and never reach into
//! the open-Trigger set.

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
use crate::listener;

/// How long a `wait_for` will poll before failing the test. Generous: it bounds a genuine
/// deadlock, not a slow machine.
const WAIT_TIMEOUT: StdDuration = StdDuration::from_secs(5);

/// A fixed, arbitrary instant. Tests advance from here explicitly; nothing depends on the real
/// clock.
pub fn test_epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()
}

pub struct TestHost {
    pub clock: Arc<TestClock>,
    socket_path: PathBuf,
    tx: Sender<Event>,
    pane: Arc<Mutex<String>>,
    /// Incremented once per draw, and the host draws once per event it processes.
    ///
    /// This is how a test knows a frame has actually been *applied* rather than merely written.
    /// Frames reach the loop through a connection thread, so without it a test that writes a
    /// frame and then advances the clock is racing its own setup — the frame can land on the
    /// far side of the advance and be stamped with the wrong open time.
    draws: Arc<AtomicU64>,
    host_thread: Option<JoinHandle<()>>,
    _dir: TempDir,
}

/// Boot a host with a temp socket, a controllable clock, and an in-memory terminal.
pub fn spawn_test_host() -> TestHost {
    spawn_test_host_with_expiry(Duration::seconds(DEFAULT_TRIGGER_EXPIRY_SECONDS))
}

pub fn spawn_test_host_with_expiry(expiry: Duration) -> TestHost {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let clock = TestClock::new(test_epoch());
    let (tx, rx) = channel();

    let listener = listener::bind(&socket_path).expect("bind test socket");
    let listener_tx = tx.clone();
    std::thread::spawn(move || listener::serve(listener, listener_tx));

    let pane = Arc::new(Mutex::new(String::new()));
    let observer_pane = Arc::clone(&pane);
    let draws = Arc::new(AtomicU64::new(0));
    let observer_draws = Arc::clone(&draws);

    let terminal = Terminal::new(TestBackend::new(60, 12)).expect("test terminal");
    let host = Host::new(terminal, clock.clone(), expiry).with_draw_observer(Box::new(
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

    pub fn key(&self, code: KeyCode) {
        self.tx
            .send(Event::Key(KeyEvent::new_with_kind(
                code,
                KeyModifiers::NONE,
                KeyEventKind::Press,
            )))
            .expect("send key");
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
