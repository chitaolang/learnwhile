//! One binary, two roles, selected by subcommand (ADR-0008).
//!
//! Argument handling is a hand-rolled match rather than `clap`: there are three subcommands with
//! no flags, and every millisecond of the `hook` path is paid by the developer on every prompt
//! submission.

use std::io::stdout;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration as StdDuration;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use learnwhile::clock::SystemClock;
use learnwhile::event::Event;
use learnwhile::frame::FrameType;
use learnwhile::host::Host;
use learnwhile::learning::Learning;
use learnwhile::listener;
use learnwhile::logging;
use learnwhile::socket::default_socket_path;
use learnwhile::storage::{NewCard, Storage, default_db_path};
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
            if let Err(error) = run_seed(&args[1..]) {
                eprintln!("learnwhile: {error:#}");
                std::process::exit(1);
            }
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
    // Open storage first, before the terminal is put into raw mode and the alternate screen: a
    // migration or config error is then reported on a normal terminal rather than from behind the
    // TUI. The expiry now lives in `config` (ADR-0006); M1's hardcoded 1800 is gone. Later M2
    // steps hand this same handle to the Learning engine — for now only the expiry is read.
    // Install the log file before anything else can fail, so a migration or bind error is on the
    // record. Held for the whole function: dropping the guard flushes buffered lines. Never on the
    // hook path (ADR-0008) — only here.
    let _log_guard = logging::init(&logging::default_log_dir());
    tracing::info!("learnwhile host starting");

    let storage = Storage::open(&default_db_path())?;
    let expiry = Duration::seconds(storage.config_i64("trigger_expiry_seconds")?);
    let clock = Arc::new(SystemClock);
    let learning = Learning::new(storage, clock.clone())?;

    let socket_path = default_socket_path();
    let listener = listener::bind(&socket_path)?;

    let (tx, rx) = channel();
    spawn_producers(listener, tx);

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let host = Host::new(terminal, clock, expiry, learning);
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
    std::thread::spawn(move || {
        listener::serve(listener, socket_tx);
        // `serve` loops forever in normal operation; returning means the accept loop died, which
        // silently starves Trigger input (ADR-0009), so it must be visible in the log.
        tracing::error!("socket accept thread exited");
    });

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
                // A read error stops key input for the rest of the run; log it rather than starving
                // the input silently (ADR-0009).
                Err(error) => {
                    tracing::error!(%error, "terminal input thread stopped");
                    return;
                }
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

/// The `seed` subcommand: ingest a tab-separated front/back file into the default deck, skipping
/// rows already present (spec §Card seeding). Deliberately TSV-only — it is a developer affordance,
/// not the deferred Anki Import/Export, and must not accrete format support.
fn run_seed(rest: &[String]) -> Result<()> {
    let path = rest.first().context("usage: learnwhile seed <file.tsv>")?;
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("reading seed file {path}"))?;
    let cards = parse_tsv(&contents);

    let mut storage = Storage::open(&default_db_path())?;
    let outcome = storage.seed_cards(&cards, Utc::now())?;

    println!(
        "{} added, {} skipped (already present)",
        outcome.added, outcome.skipped
    );
    Ok(())
}

/// Parse front/back pairs, one per line, split on the first tab. Lines with no tab (including
/// blanks) and rows with an empty front or back are skipped rather than aborting the import, so one
/// typo does not cost the whole file. Splitting on the first tab means a back field may contain tabs.
fn parse_tsv(contents: &str) -> Vec<NewCard> {
    contents
        .lines()
        .filter_map(|line| {
            let (front, back) = line.split_once('\t')?;
            let front = front.trim();
            let back = back.trim();
            if front.is_empty() || back.is_empty() {
                return None;
            }
            Some(NewCard {
                front: front.to_string(),
                back: back.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tsv_skips_blank_and_tabless_lines() {
        let cards = parse_tsv("front\tback\n\nno tab here\n   \nq\ta\n");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].front, "front");
        assert_eq!(cards[0].back, "back");
        assert_eq!(cards[1].front, "q");
    }

    #[test]
    fn parse_tsv_splits_on_the_first_tab_so_a_back_may_contain_tabs() {
        let cards = parse_tsv("f\tb1\tb2\n");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].back, "b1\tb2");
    }

    #[test]
    fn parse_tsv_skips_rows_with_an_empty_side() {
        let cards = parse_tsv("\tonly back\nfront only\t\n");
        assert!(cards.is_empty());
    }
}
