use futures::prelude::*;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};

use tokio::timer::Delay;

// Duration newtypes
pub struct BaseTime(pub Duration);
pub struct TimePerTurn(pub Duration);

/// There's probably a design involving a cycle iterator and
/// `[rental](https://crates.io/crates/rental)`, but that seems like overengineering at this point
struct ClockCore {
    remaining: Vec<Duration>,
    n_players: usize,
    active: usize,
    time_per_turn: Duration,
    time_last_triggered: Option<Instant>,
}

impl ClockCore {
    /// If passed 0 players for whatever reason, it should panic somewhere down the line.
    fn new(
        n_players: usize,
        base_time: BaseTime,
        time_per_turn: TimePerTurn,
    ) -> ClockCore {
        ClockCore {
            remaining: vec![base_time.0 + time_per_turn.0; n_players],
            n_players: n_players,
            active: 0,
            time_per_turn: time_per_turn.0,
            time_last_triggered: None,
        }
    }
}

pub struct ChessClock(Arc<Mutex<ClockCore>>);

impl ChessClock {
    pub fn new(
        n_players: usize,
        base_time: BaseTime,
        time_per_turn: TimePerTurn,
    ) -> ChessClock {
        ChessClock(Arc::new(Mutex::new(ClockCore::new(n_players, base_time, time_per_turn))))
    }

    // Not sure if it's possible to implement this as a method taking &mut self
    pub fn bind<F>(self, f: F) -> ClockedFuture<F::Future>
    where F: IntoFuture<Error = ()> {
        let now = Instant::now();
        let mut expiry_time = now;
        {
            let clock = &mut *self.0.lock().unwrap();
            let duration = clock.remaining[clock.active];
            clock.time_last_triggered = Some(now);
            expiry_time += duration;
        }
        let expire = Box::new(Delay::new(expiry_time)
                .map_err(|_| ()));
        ClockedFuture {
            clock: self,
            expire: expire,
            future: f.into_future(),
        }
    }
}

pub struct ClockedFuture<F> {
    clock: ChessClock,
    expire: Box<Future<Item=(), Error=()> + Send>,
    future: F,
}

impl<F> Future for ClockedFuture<F>
where F: Future<Error = ()> {
    type Item = Option<F::Item>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = {
            match self.future.poll()? {
                Async::Ready(item) => {
                    let mut clock = &mut *self.clock.0.lock().unwrap();
                    let active_player_remaining = &mut clock.remaining[clock.active];
                    let new_remaining = *active_player_remaining + clock.time_per_turn
                        - clock.time_last_triggered.unwrap().elapsed();
                    *active_player_remaining = new_remaining;

                    if clock.active == clock.n_players - 1 {
                        clock.active = 0;
                    } else {
                        clock.active += 1;
                    }
                    Some(item)
                },
                Async::NotReady => {
                    match self.expire.poll()? {
                        Async::Ready(()) => None,
                        Async::NotReady => return Ok(Async::NotReady),
                    }
                }
            }
        };
        Ok(Async::Ready(item))
    }
}
