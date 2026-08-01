//! The socket accept thread — one of the three producers feeding the event loop (ADR-0009).

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::event::Event;
use crate::frame::{DiscardReason, FrameType, MAX_LINE_BYTES, TriggerFrame, Verdict, parse_line};

/// How long a connected client has to send its line before we give up on it. Bounds a client
/// that connects and then says nothing, which would otherwise hold a connection thread forever.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait on the event loop for a gate verdict before failing open. Generous: the host
/// answers a gate query almost immediately, so this only bounds a wedged loop, and a missed verdict
/// becomes an `Allow` so the developer's prompt is never held by our own slowness (ADR-0004).
const GATE_REPLY_TIMEOUT: Duration = Duration::from_secs(1);

/// Bind the socket, recovering a stale file and refusing a live one.
///
/// A socket file with a *live* listener is never unlinked: we probe it with a connect, and a
/// successful connect means another host owns it (ADR-0003), so we refuse with a clear message. A
/// file with no listener is stale (a killed host left it) and is unlinked before rebinding. These
/// are the same code path from opposite directions.
pub fn bind(socket_path: &Path) -> Result<UnixListener> {
    if socket_path.exists() {
        // A successful connect means a live host owns the socket (ADR-0003). Refuse with a message
        // that says what is running and what to do, rather than a raw bind error. The connect is a
        // courtesy for the message; the bind below is the authoritative serialization point, so two
        // hosts racing to start still cannot both win (milestone risk note).
        if UnixStream::connect(socket_path).is_ok() {
            anyhow::bail!(
                "another LearnWhile host is already listening on {}. Only one host may run at a \
                 time, so stop the other before starting this one.",
                socket_path.display()
            );
        }
        // No listener answered, so the file is stale (a killed host leaves it behind). Unlink it.
        std::fs::remove_file(socket_path)
            .with_context(|| format!("removing stale socket at {}", socket_path.display()))?;
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    UnixListener::bind(socket_path)
        .with_context(|| format!("binding socket at {}", socket_path.display()))
}

/// Accept forever, handing each connection to its own thread.
///
/// The failure boundary is the connection, not the thread: a malformed frame may never kill the
/// accept loop (ADR-0007).
pub fn serve(listener: UnixListener, tx: Sender<Event>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = tx.clone();
                std::thread::spawn(move || handle_connection(stream, tx));
            }
            // An accept error is not fatal; the next accept may well succeed.
            Err(error) => {
                tracing::warn!(%error, "accept failed; continuing");
                continue;
            }
        }
    }
}

fn handle_connection(stream: UnixStream, tx: Sender<Event>) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut reader = BufReader::new(stream);

    loop {
        let mut line = Vec::new();
        // A fresh `take` per line, so the cap applies per line rather than per connection.
        let read = reader
            .by_ref()
            .take(MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line);

        match read {
            Ok(0) => return,
            Ok(_) => {}
            Err(_) => return,
        }

        if !line.ends_with(b"\n") {
            // Either an oversized line or a truncated one. Both are discards; neither is worth
            // trying to resynchronise from, because we cannot tell where the next line starts.
            discard(DiscardReason::OversizedLine);
            return;
        }

        match std::str::from_utf8(&line) {
            Ok(text) => match parse_line(text.trim_end_matches('\n')) {
                // A gate query needs a verdict written back on this same connection (ADR-0016),
                // so it does not go down the fire-and-forget `Event::Frame` path.
                Ok(frame) if frame.frame_type == FrameType::GateQuery => {
                    if !answer_gate_query(reader.get_mut(), &frame, &tx) {
                        return;
                    }
                }
                Ok(frame) => {
                    if tx.send(Event::Frame(frame)).is_err() {
                        // The host is gone; nothing left to serve.
                        return;
                    }
                }
                Err(reason) => discard(reason),
            },
            Err(_) => discard(DiscardReason::Unparseable),
        }
    }
}

/// Ask the event loop for a gate verdict and write it back on `stream`. Returns false when the host
/// is gone, so the caller stops serving. A verdict that does not arrive in time is treated as
/// `Allow`: our own slowness must never hold the developer's prompt (ADR-0004).
fn answer_gate_query(stream: &mut UnixStream, frame: &TriggerFrame, tx: &Sender<Event>) -> bool {
    let (reply_tx, reply_rx) = channel();
    if tx
        .send(Event::GateQuery {
            key: frame.key(),
            reply: reply_tx,
        })
        .is_err()
    {
        return false;
    }
    let verdict = reply_rx
        .recv_timeout(GATE_REPLY_TIMEOUT)
        .unwrap_or(Verdict::Allow);
    stream.write_all(verdict.to_line().as_bytes()).is_ok()
}

/// Drop a frame, recording why. ADR-0007 requires this log precisely because a silent discard
/// makes an adapter bug invisible — the host stays fail-open, so the log is the only trace.
fn discard(reason: DiscardReason) {
    tracing::warn!(?reason, "discarded trigger frame");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A `MakeWriter` that captures log output into a shared buffer, so a scoped subscriber can
    /// assert what `discard` emitted without touching the global logger.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl tracing_subscriber::fmt::MakeWriter<'_> for Capture {
        type Writer = Capture;
        fn make_writer(&self) -> Self::Writer {
            self.clone()
        }
    }

    fn discard_log(reason: DiscardReason) -> String {
        let capture = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, || discard(reason));
        String::from_utf8(capture.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn every_discard_reason_is_logged_with_its_reason() {
        assert!(discard_log(DiscardReason::Unparseable).contains("Unparseable"));
        assert!(discard_log(DiscardReason::UnknownVersion(99)).contains("UnknownVersion(99)"));
        let oversized = discard_log(DiscardReason::OversizedLine);
        assert!(oversized.contains("OversizedLine"));
        // The human-readable message is there too, not just the machine field.
        assert!(oversized.contains("discarded trigger frame"));
    }

    #[test]
    fn bind_refuses_a_live_host_and_leaves_its_socket_untouched() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("host.sock");

        let _first = bind(&path).expect("first host binds");

        // A second host must refuse, name the socket, and must NOT unlink the live one.
        let error = bind(&path).expect_err("second host must refuse");
        assert!(
            error.to_string().contains("already listening"),
            "unexpected message: {error}"
        );
        assert!(path.exists(), "the live socket must not be unlinked");
        assert!(
            UnixStream::connect(&path).is_ok(),
            "the first host must still be listening"
        );
    }

    #[test]
    fn bind_recovers_from_a_stale_socket_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("host.sock");

        // A killed host leaves the socket file behind with no listener (dropping a UnixListener does
        // not unlink it).
        let dead = UnixListener::bind(&path).expect("first bind");
        drop(dead);
        assert!(
            path.exists(),
            "a stale socket file remains after the listener drops"
        );

        // Recovery: the connect probe fails, so bind unlinks the stale file and rebinds cleanly.
        let _live = bind(&path).expect("bind recovers from a stale socket");
        assert!(UnixStream::connect(&path).is_ok());
    }
}
