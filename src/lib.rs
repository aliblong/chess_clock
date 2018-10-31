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
        use tokio;
        use super::{ChessClock, BaseTime, TimePerTurn};
        use futures::prelude::*;
        use futures::sync::mpsc::unbounded;
        use std::thread::sleep;
        use std::time::{Duration, Instant};
        use tokio::timer::Delay;
        use futures::future::{self, ok, err, Either, FutureResult};
        use tokio_current_thread;

        let mut chess_clock = ChessClock::new(
            2,
            BaseTime(Duration::new(5, 0)),
            TimePerTurn(Duration::new(5, 0)),
        );

        let when = Instant::now() + Duration::from_millis(1000);
        let task = Delay::new(when)
            .map_err(|_| ());
        let clocked_task = chess_clock.bind(task)
            .then(|res| {
                match res {
                    Ok(Either::A(_)) => {
                        println!("Task succeeded");
                        ok(())
                    },
                    Ok(Either::B(_)) => {
                        println!("Task timed out");
                        ok(())
                    },
                    _ => {
                        err(())
                    }
                }
            }
            );
        tokio::run(clocked_task);
    }
}
