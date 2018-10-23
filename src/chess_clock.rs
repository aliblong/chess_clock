use std::time::{Duration, Instant};
use std::iter::Cycle;
use tokio;
use tokio::prelude::*;
use tokio::timer::Delay;
use std::thread;

/// There's probably a design involving a cycle iterator and
/// `[rental](https://crates.io/crates/rental)`, but that seems like overengineering at this point
pub struct ChessClock {
    remaining: Vec<Duration>,
    n_players: usize,
    active: usize,
    time_per_turn: Duration,
    time_last_triggered: Option<Instant>,
    clock: Option<Delay>,
}

// Duration newtypes
pub struct BaseTime(pub Duration);
pub struct TimePerTurn(pub Duration);

impl ChessClock {
    /// If passed 0 players for whatever reason, it should panic somewhere down the line.
    /// Per FIDE rules, time per turn is added on the very first move
    pub fn new(n_players: usize, base_time: BaseTime, time_per_turn: TimePerTurn) -> ChessClock {
        ChessClock {
            remaining: vec![base_time.0 + time_per_turn.0; n_players],
            n_players: n_players,
            active: 0,
            time_per_turn: time_per_turn.0,
            time_last_triggered: None,
            clock: None,
        }
    }
    pub fn start() {
    }
    fn launch_timer(&mut self, duration: Duration) {
        let now = Instant::now();
        let expiry_time = now + duration;
        let trigger_expiry = Delay::new(expiry_time)
            .and_then(|_| {
                self.expire();
                Ok(())
            })
            .map_err(|e| panic!("delay errored; err={:?}", e));
        self.time_last_triggered = Some(now);
        self.clock = Some(trigger_expiry);

        thread::spawn(move || {
            tokio::run(trigger_expiry);
        }
    }
    fn stop_timer(&mut self) {}
    fn expire(&mut self) {}
    pub fn next(&mut self) -> Duration {
        let active_player_remaining = self.remaining[self.active];
        let new_remaining =
            active_player_remaining +
            self.time_per_turn -
            self.time_last_triggered.unwrap().elapsed();
        active_player_remaining = new_remaining;

        if self.active == self.n_players - 1 {
            self.active = 0;
        }
        else {
            self.active += 1;
        }
        active_player_remaining = self.remaining[self.active];
        self.launch_timer(new_remaining);

        return new_remaining;
    }
}
