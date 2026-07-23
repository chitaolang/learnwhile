//! The socket accept thread — one of the three producers feeding the event loop (ADR-0009).

use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::event::Event;
use crate::frame::{DiscardReason, MAX_LINE_BYTES, parse_line};

/// How long a connected client has to send its line before we give up on it. Bounds a client
/// that connects and then says nothing, which would otherwise hold a connection thread forever.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Bind the socket, clearing a stale file first.
///
/// A socket file with a *live* listener is never unlinked: we probe it with a connect, and a
/// successful connect means another host owns it (ADR-0003). M5 turns this into a proper
/// single-instance message; here it is the minimum needed not to stomp a running host.
pub fn bind(socket_path: &Path) -> Result<UnixListener> {
    if socket_path.exists() {
        if UnixStream::connect(socket_path).is_ok() {
            anyhow::bail!(
                "another LearnWhile host is already listening on {}",
                socket_path.display()
            );
        }
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
            Err(_) => continue,
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

/// Drop a frame. Silent for now — M5 gives this a log file, which ADR-0007 requires precisely
/// because silent discards make adapter bugs invisible.
fn discard(_reason: DiscardReason) {}
