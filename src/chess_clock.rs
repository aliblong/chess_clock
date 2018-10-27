use futures::future::{self, ok, err, Either, FutureResult};
use futures::prelude::*;
use std::iter::Cycle;
use futures::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use tokio;
use tokio::prelude::*;
use tokio::timer::Delay;

/// There's probably a design involving a cycle iterator and
/// `[rental](https://crates.io/crates/rental)`, but that seems like overengineering at this point
pub struct ChessClock {
    remaining: Vec<Duration>,
    n_players: usize,
    active: usize,
    time_per_turn: Duration,
    new_duration_tx: Sender<Duration>,
    timeout_tx: Sender<()>,
    next_rx: Receiver<()>,
    time_last_triggered: Option<Instant>,
    //clock: Option<Delay>,
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
        new_duration_tx: Sender<Duration>,
        timeout_tx: Sender<()>,
        next_rx: Receiver<()>,
    ) -> ChessClock {
        ChessClock {
            remaining: vec![base_time.0 + time_per_turn.0; n_players],
            n_players: n_players,
            active: 0,
            time_per_turn: time_per_turn.0,
            new_duration_tx: new_duration_tx,
            timeout_tx: timeout_tx,
            next_rx: next_rx,
            time_last_triggered: None,
        }
    }

    pub fn start() {}

    fn recv_next(self) -> FutureResult<(), ()> {
        self.next_rx.wait();
        ok(())
    }

    fn launch_timer(&mut self, duration: Duration) {
        let now = Instant::now();
        let expiry_time = now + duration;
        self.time_last_triggered = Some(now);
        let future_expire =
            Delay::new(expiry_time)
                .map_err(|_| ())
                .into_stream();
        //let future_recv_next = self.recv_next();
        //let future_recv_next = self.next_rx;
        let next_or_expire = StreamNext::new(self.next_rx, future_expire)
            .then(|res| {
                match res {
                    Ok(Either::A(_)) => {
                        self.expire();
                        ok(())
                    },
                    Ok(Either::B(_)) => {
                        self.next();
                        ok(())
                    },
                    _ => {
                        self.error();
                        ok(())
                    }
                }
            });
        tokio::run(next_or_expire);
    }

    fn stop_timer(&mut self) {}
    fn expire(&mut self) {}
    fn error(&mut self) {}

    pub fn next(&mut self) {
        let active_player_remaining = self.remaining[self.active];
        let new_remaining = active_player_remaining + self.time_per_turn
            - self.time_last_triggered.unwrap().elapsed();
        active_player_remaining = new_remaining;
        self.new_duration_tx.send(new_remaining);

        if self.active == self.n_players - 1 {
            self.active = 0;
        } else {
            self.active += 1;
        }
        active_player_remaining = self.remaining[self.active];
        self.launch_timer(active_player_remaining);
    }
}

/// Helper future to wait for the first item on one of two streams, returning
/// the item and the two streams when done.
struct StreamNext<S1, S2> {
    left: S1,
    right: S2,
}

impl<S1, S2> StreamNext<S1, S2>
where
    S1: Stream,
    S2: Stream<Error = S1::Error>,
{
    fn new(s1: S1, s2: S2) -> StreamNext<S1, S2> {
        StreamNext {
            left: s1,
            right: s2,
        }
    }
}

impl<S1, S2> Future for StreamNext<S1, S2>
where
    S1: Stream,
    S2: Stream<Error = S1::Error>,
{
    type Item = Either<S1::Item, S2::Item>;
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = {
            match self.left.poll()? {
                Async::Ready(item) => Either::A(item.unwrap()),
                Async::NotReady => {
                    match self.right.poll()? {
                        Async::Ready(item) => Either::B(item.unwrap()),
                        Async::NotReady => return Ok(Async::NotReady),
                    }
                }
            }
        };
        Ok(Async::Ready(item))
    }
}
