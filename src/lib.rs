extern crate futures;
extern crate stream_cancel;
extern crate tokio;
extern crate tokio_current_thread;

mod chess_clock;
pub use chess_clock::{ChessClock, BaseTime, TimePerTurn};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        use tokio::{run, spawn};
        use super::{ChessClock, BaseTime, TimePerTurn};
        use futures::prelude::*;
        use futures::sync::mpsc::unbounded;
        use std::thread::sleep;
        use std::time::{Duration, Instant};
        use tokio::timer::Delay;
        use futures::future::{self, ok, err, Either, FutureResult};
        use tokio_current_thread::block_on_all;

        let (duration_tx, duration_rx) = unbounded();
        let (expire_tx, expire_rx) = unbounded();
        let (next_tx, next_rx) = unbounded();
        let mut chess_clock = ChessClock::new(
            2,
            BaseTime(Duration::new(5, 0)),
            TimePerTurn(Duration::new(5, 0)),
            duration_tx,
            expire_tx,
            next_rx,
        );

	let clock = chess_clock.start();
	let when = Instant::now() + Duration::from_millis(1000);
	let task = Delay::new(when)
	    .and_then(|_| {
		println!("{:?}", duration_rx.wait());
		next_tx.send(());
		let now = Instant::now();
		expire_rx.wait();
		println!("{:?}", now.elapsed().as_secs());
		ok(())
	    })
	    .map_err(|_| ());
        let supertask = clock.join(task);
        block_on_all(supertask);
    }
}
