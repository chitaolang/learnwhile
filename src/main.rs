//! One binary, two roles, selected by subcommand (ADR-0008).
//!
//! Argument handling is a hand-rolled match rather than `clap`: there are three subcommands with
//! no flags, and every millisecond of the `hook` path is paid by the developer on every prompt
//! submission.

use std::io::stdout;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration as StdDuration;

use anyhow::Result;
use chrono::Duration;
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use learnwhile::clock::SystemClock;
use learnwhile::event::Event;
use learnwhile::frame::FrameType;
use learnwhile::host::{DEFAULT_TRIGGER_EXPIRY_SECONDS, Host};
use learnwhile::listener;
use learnwhile::socket::default_socket_path;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// How often the sweep fires. Independent of frame arrival, per ADR-0006.
const TICK_INTERVAL: StdDuration = StdDuration::from_secs(30);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);

    match command {
        // The hook path. Nothing may be initialised before this branch is taken.
        Some("hook") => {
            run_hook(&args[1..]);
            std::process::exit(0);
        }
        Some("seed") => {
            eprintln!("`seed` arrives in M2 — see docs/milestones/M2-cards-and-reviews.md");
            std::process::exit(1);
        }
        None | Some("host") => {
            if let Err(error) = run_host() {
                eprintln!("learnwhile: {error:#}");
                std::process::exit(1);
            }
        }
        Some(other) => {
            eprintln!("learnwhile: unknown subcommand {other:?}");
            eprintln!("usage: learnwhile [host|hook|seed]");
            std::process::exit(2);
        }
    }
}

/// The Trigger Adapter. Exits 0 whatever happens, including on a panic (ADR-0004).
fn run_hook(rest: &[String]) {
    let forced = match rest.first().map(String::as_str) {
        Some("--open") => Some(FrameType::TriggerOpen),
        Some("--close") => Some(FrameType::TriggerClose),
        _ => None,
    };

    // Swallow a panic's output as well as the panic: a backtrace on stderr during a hook would
    // be noise in the developer's agent session.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(|| {
        learnwhile::hook::run(&default_socket_path(), forced);
    });
    std::panic::set_hook(previous);
}

fn run_host() -> Result<()> {
    let socket_path = default_socket_path();
    let listener = listener::bind(&socket_path)?;

    let (tx, rx) = channel();
    spawn_producers(listener, tx);

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let host = Host::new(
        terminal,
        Arc::new(SystemClock),
        Duration::seconds(DEFAULT_TRIGGER_EXPIRY_SECONDS),
    );
    let result = host.run(rx);

    // Restore the terminal before reporting, so an error is readable rather than drawn over the
    // alternate screen.
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Best-effort: the next host unlinks a stale socket anyway, but leaving one behind for a
    // process that exited cleanly is untidy.
    let _ = std::fs::remove_file(&socket_path);

    result
}

/// The three producers of ADR-0009. Each translates one source into an `Event`; none holds state.
fn spawn_producers(listener: std::os::unix::net::UnixListener, tx: Sender<Event>) {
    let socket_tx = tx.clone();
    std::thread::spawn(move || listener::serve(listener, socket_tx));

    let input_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(key)) => {
                    if input_tx.send(Event::Key(key)).is_err() {
                        return;
                    }
                }
                Ok(_) => continue,
                Err(_) => return,
            }
        }
    });

    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK_INTERVAL);
            if tx.send(Event::Tick).is_err() {
                return;
            }
        }
    });
}
