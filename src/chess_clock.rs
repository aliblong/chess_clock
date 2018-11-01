//! `ChessClock` is ARCed and mutexed so that it works nicely with the borrow checker, but by
//! nature of its design, which is sequential passing of turns, it shouldn't be used simultaneously
//! by two threads.
//!
//! It tracks the time of each player, a single global time-per-turn, the active player index, and
//! the time the last turn was passed, so it can calculate the time to subtract from the next
//! active player's clock.

use futures::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::timer::Delay;

/// Duration newtype
pub struct BaseTime(pub Duration);
/// Duration newtype
pub struct TimePerTurn(pub Duration);

struct ClockCore {
    remaining: Vec<Duration>,
    n_players: usize,
    // There's probably a design involving a cycle iterator and
    // `[rental](https://crates.io/crates/rental)` rather than just keeping track of the active player
    // index, but that seems like overengineering at this point.
    active: usize,
    time_per_turn: Duration,
    time_last_triggered: Option<Instant>,
}

impl ClockCore {
    fn new(n_players: usize, base_time: BaseTime, time_per_turn: TimePerTurn) -> ClockCore {
        ClockCore {
            remaining: vec![base_time.0 + time_per_turn.0; n_players],
            n_players: n_players,
            active: 0,
            time_per_turn: time_per_turn.0,
            time_last_triggered: None,
        }
    }
    fn next(&mut self) {
        let active_player_remaining = &mut self.remaining[self.active];
        let new_remaining =
            active_player_remaining.checked_sub(self.time_last_triggered.unwrap().elapsed());
        *active_player_remaining = match new_remaining {
            Some(remaining) => remaining,
            None => Duration::new(0, 0),
        };
        *active_player_remaining += self.time_per_turn;

        if self.active == self.n_players - 1 {
            self.active = 0;
        } else {
            self.active += 1;
        }
    }
}

#[derive(Clone)]
pub struct ChessClock(Arc<Mutex<ClockCore>>);

impl ChessClock {
    /// Do not try to make a chess clock with 0 players, or it will panic somewhere down the line.
    pub fn new(n_players: usize, base_time: BaseTime, time_per_turn: TimePerTurn) -> ChessClock {
        ChessClock(Arc::new(Mutex::new(ClockCore::new(
            n_players,
            base_time,
            time_per_turn,
        ))))
    }

    /// Combine the clock with another future `f` to produce a
    /// [`ClockedFuture`](ClockedFuture).
    pub fn bind<F>(self, f: F) -> ClockedFuture<F::Future>
    where
        F: IntoFuture<Error = ()>,
    {
        let now = Instant::now();
        let mut expiry_time = now;
        {
            let clock = &mut *self.0.lock().unwrap();
            let duration = clock.remaining[clock.active];
            clock.time_last_triggered = Some(now);
            expiry_time += duration;
        }
        let expire = Box::new(Delay::new(expiry_time).map_err(|_| ()));
        ClockedFuture {
            clock: self,
            expire: expire,
            future: f.into_future(),
        }
    }

    pub fn active_player_time_remaining(&self) -> Duration {
        let clock = &*self.0.lock().unwrap();
        clock.remaining[clock.active]
    }
    pub fn times_remaining(&self) -> Vec<Duration> {
        let clock = &*self.0.lock().unwrap();
        clock.remaining.clone()
    }
    pub fn active_player(&self) -> usize {
        let clock = &*self.0.lock().unwrap();
        clock.active
    }
}

/// Represents the combination of a future `f` with a chess clock through
/// [`bind`](ChessClock::bind).
///
/// A `ClockedFuture` will return either `Some(item)`, where `item` is the return value of `f`,
/// or `None`, if the clock expires.
pub struct ClockedFuture<F> {
    clock: ChessClock,
    expire: Box<Future<Item = (), Error = ()> + Send>,
    future: F,
}

/// If the future returns before the clock expires, the chess clock subtracts the elapsed time from
/// the active player's clock and adds the time-per-turn. Then, the next player becomes the active
/// player.
impl<F> Future for ClockedFuture<F>
where
    F: Future<Error = ()>,
{
    type Item = Option<F::Item>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = {
            match self.future.poll()? {
                Async::Ready(item) => Some(item),
                Async::NotReady => match self.expire.poll()? {
                    Async::Ready(()) => None,
                    Async::NotReady => return Ok(Async::NotReady),
                },
            }
        };
        let clock = &mut *self.clock.0.lock().unwrap();
        clock.next();
        Ok(Async::Ready(item))
    }
}
