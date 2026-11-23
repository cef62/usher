//! The engine thread: one channel in, `handle` for events, `tick` when a deadline passes.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use log::debug;
use usher_coreaudio::{AudioSystem, Notify};

use super::{Engine, Event, Flow, Shell};

#[derive(Clone)]
pub struct EngineHandle(Sender<Event>);

impl EngineHandle {
    pub fn new() -> (EngineHandle, Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        (EngineHandle(tx), rx)
    }

    /// `false` when the engine is gone.
    pub fn send(&self, ev: Event) -> bool {
        let ok = self.0.send(ev).is_ok();
        if !ok {
            debug!("engine is gone; event dropped");
        }
        ok
    }

    pub fn notify(&self) -> Notify {
        let h = self.clone();
        Arc::new(move |ev| {
            h.send(Event::Audio(ev));
        })
    }

    /// Ask the engine to flush and stop; `true` once it acknowledged within `timeout`.
    pub fn quit(&self, timeout: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        self.send(Event::Quit(tx)) && rx.recv_timeout(timeout).is_ok()
    }
}

pub fn run<A: AudioSystem, S: Shell>(mut engine: Engine<A, S>, rx: Receiver<Event>) {
    loop {
        let got = match engine.next_due() {
            Some(d) => rx.recv_timeout(d.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match got {
            Ok(ev) => {
                if engine.handle(ev, Instant::now()) == Flow::Stop {
                    return;
                }
                // a steady event backlog must not starve the deadline
                if engine.next_due().is_some_and(|d| d <= Instant::now()) {
                    engine.tick(Instant::now());
                }
            }
            Err(RecvTimeoutError::Timeout) => engine.tick(Instant::now()),
            Err(RecvTimeoutError::Disconnected) => {
                engine.shutdown();
                return;
            }
        }
    }
}
