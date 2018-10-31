use futures::future::{self, ok, err, Either, FutureResult};
use futures::prelude::*;
use std::iter::Cycle;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use std::thread;
use std::time::{Duration, Instant};
use tokio;
use tokio_current_thread::block_on_all;
use tokio::prelude::*;
use tokio::timer::Delay;

enum Action {
    Next,
    Expire,
}

/// There's probably a design involving a cycle iterator and
/// `[rental](https://crates.io/crates/rental)`, but that seems like overengineering at this point
pub struct ChessClock {
    remaining: Vec<Duration>,
    n_players: usize,
    active: usize,
    time_per_turn: Duration,
    time_last_triggered: Option<Instant>,
}

pub struct ClockedFuture<'a, F> {
    clock: &'a mut ChessClock,
    expire: Box<Future<Item=(), Error=()> + Send>,
    future: F,
}

// Duration newtypes
pub struct BaseTime(pub Duration);
pub struct TimePerTurn(pub Duration);


impl ChessClock {
    /// If passed 0 players for whatever reason, it should panic somewhere down the line.
    /// Per FIDE rules, time per turn is added on the very first move
    pub fn new(
        n_players: usize,
        base_time: BaseTime,
        time_per_turn: TimePerTurn,
    ) -> ChessClock {
        ChessClock {
            remaining: vec![base_time.0 + time_per_turn.0; n_players],
            n_players: n_players,
            active: 0,
            time_per_turn: time_per_turn.0,
            time_last_triggered: None,
        }
    }

    // Not sure if it's possible to implement this as a method taking &mut self
    pub fn bind<F>(&mut self, f: F) -> ClockedFuture<F::Future>
    where F: IntoFuture<Error = ()> {
        let duration = self.remaining[self.active];
        let now = Instant::now();
        let expiry_time = now + duration;
        self.time_last_triggered = Some(now);
        let expire = Box::new(Delay::new(expiry_time)
                .map_err(|_| ()));
        ClockedFuture {
            clock: self,
            expire: expire,
            future: f.into_future(),
        }
    }
}

impl<'a, F> Future for ClockedFuture<'a, F>
where F: Future<Error = ()> {
    type Item = Either<F::Item, ()>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = {
            match self.future.poll()? {
                Async::Ready(item) => {
                    let clock = &mut self.clock;
                    let active_player_remaining = &mut clock.remaining[clock.active];
                    let new_remaining = *active_player_remaining + clock.time_per_turn
                        - clock.time_last_triggered.unwrap().elapsed();
                    *active_player_remaining = new_remaining;

                    if clock.active == clock.n_players - 1 {
                        clock.active = 0;
                    } else {
                        clock.active += 1;
                    }
                    Either::A(item)
                },
                Async::NotReady => {
                    match self.expire.poll()? {
                        Async::Ready(()) => Either::B(()),
                        Async::NotReady => return Ok(Async::NotReady),
                    }
                }
            }
        };
        Ok(Async::Ready(item))
    }
}
