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
    new_duration_tx: UnboundedSender<Duration>,
    expire_tx: UnboundedSender<()>,
    next_rx: Option<UnboundedReceiver<()>>,
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
        new_duration_tx: UnboundedSender<Duration>,
        expire_tx: UnboundedSender<()>,
        next_rx: UnboundedReceiver<()>,
    ) -> ChessClock {
        ChessClock {
            remaining: vec![base_time.0 + time_per_turn.0; n_players],
            n_players: n_players,
            active: 0,
            time_per_turn: time_per_turn.0,
            new_duration_tx: new_duration_tx,
            expire_tx: expire_tx,
            next_rx: Some(next_rx),
            time_last_triggered: None,
        }
    }

    fn launch_timer(&mut self, duration: Duration) -> Result<Action, ()> {
        let now = Instant::now();
        let expiry_time = now + duration;
        self.time_last_triggered = Some(now);
        let future_expire =
            Delay::new(expiry_time)
                .map_err(|_| ())
                .into_stream();
        //let future_recv_next = self.recv_next();
        //let future_recv_next = self.next_rx;
        let next_or_expire = StreamNext::new(self.next_rx.take().unwrap(), future_expire)
            .then(|res| {
                match res {
                    Ok(Some((Either::A(_), rx, timer))) => {
                        self.next_rx = Some(rx);
                        drop(timer);
                        ok(Action::Next)
                    },
                    Ok(Some((Either::B(_), rx, _))) => {
                        self.next_rx = Some(rx);
                        ok(Action::Expire)
                    },
                    _ => {
                        err(())
                    }
                }
            });
        block_on_all(next_or_expire)
    }

    pub fn start(&mut self) -> impl Future<Item = (), Error = ()> {
        let start_duration = self.remaining[0];
        self.process(start_duration);
        ok(())
    }

    fn process(&mut self, duration: Duration) {
        match self.launch_timer(duration) {
            Ok(Action::Next) => self.next(),
            Ok(Action::Expire) => self.expire(),
            Err(()) => self.error(),
        }
    }

    fn expire(&mut self) {
        self.expire_tx.clone().send(());
    }
    fn error(&mut self) {}

    pub fn next(&mut self) {
        {
            let active_player_remaining = &mut self.remaining[self.active];
            let new_remaining = *active_player_remaining + self.time_per_turn
                - self.time_last_triggered.unwrap().elapsed();
            *active_player_remaining = new_remaining;
            // This clone is needed because of how UnboundedSender is defined, I think
            self.new_duration_tx.clone().send(new_remaining);
        }

        if self.active == self.n_players - 1 {
            self.active = 0;
        } else {
            self.active += 1;
        }
        let active_player_remaining = self.remaining[self.active];
        self.process(active_player_remaining);
    }
}

/// https://github.com/rust-lang-nursery/futures-rs/issues/315#issuecomment-325061072
/// Helper future to wait for the first item on one of two streams, returning
/// the item and the two streams when done.
struct StreamNext<S1, S2> {
    left: Option<S1>,
    right: Option<S2>,
}

impl<S1, S2> StreamNext<S1, S2>
where
    S1: Stream,
    S2: Stream<Error = S1::Error>,
{
    fn new(s1: S1, s2: S2) -> StreamNext<S1, S2> {
        StreamNext {
            left: Some(s1),
            right: Some(s2),
        }
    }
}

impl<S1, S2> Future for StreamNext<S1, S2>
where
    S1: Stream,
    S2: Stream<Error = S1::Error>,
{
    type Item = Option<(Either<S1::Item, S2::Item>, S1, S2)>;
    type Error = S1::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = {
            let left = self.left.as_mut().unwrap();
            let right = self.right.as_mut().unwrap();
            match left.poll()? {
                Async::Ready(None) => return Ok(Async::Ready(None)),
                Async::Ready(Some(item)) => Either::A(item),
                Async::NotReady => {
                    match right.poll()? {
                        Async::Ready(None) => return Ok(Async::Ready(None)),
                        Async::Ready(Some(item)) => Either::B(item),
                        Async::NotReady => return Ok(Async::NotReady),
                    }
                }
            }
        };
        Ok(Async::Ready(Some((
            item,
            self.left.take().unwrap(),
            self.right.take().unwrap(),
        ))))
    }
}
