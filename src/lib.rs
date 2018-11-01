//! A futures-based chess clock which provides timeout-like functionality for other futures.
//!
//! In addition to the typical configurable base time and time-per-turn, `ChessClock` supports [1,
//! usize::max] players, which can be useful in time-controlled, turn-based games that support more
//! than two players, such as [Hanabi](https://hanabi.live).
//!
//! Per FIDE rules, time-per-turn is added on the very first move.
//!
//! # Example usage:
//! ```
//!   extern crate futures;
//!   extern crate tokio;
//!   extern crate chess_clock;
//!
//!   use chess_clock::{ChessClock, BaseTime, TimePerTurn};
//!
//!   use std::thread::sleep;
//!   use std::time::{Duration, Instant};
//!
//!   use futures::{prelude::*, future::{self, ok, err, Either, FutureResult}};
//!   use tokio;
//!   use tokio::timer::Delay;
//!
//!   let mut clock = ChessClock::new(
//!       2,
//!       BaseTime(Duration::new(2, 0)),
//!       TimePerTurn(Duration::new(2, 0)),
//!   );
//!
//!   let when = Instant::now() + Duration::from_millis(2000);
//!   let task = Delay::new(when)
//!       .map_err(|_| ());
//!   let clocked_task = clock.bind(task)
//!       .then(|res| {
//!           match res {
//!               Ok(Some(_)) => {
//!                   println!("Task succeeded");
//!                   ok(())
//!               },
//!               Ok(None) => {
//!                   println!("Task timed out");
//!                   ok(())
//!               },
//!               _ => {
//!                   err(())
//!               }
//!           }
//!       }
//!       );
//!   tokio::run(clocked_task);
//! ```
//! Output: `Task succeeded`, since the example task has duration less than the first player's base
//! time + time-per-turn

extern crate futures;
extern crate tokio;

mod chess_clock;
pub use chess_clock::{ChessClock, ClockedFuture, BaseTime, TimePerTurn};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        use super::{ChessClock, BaseTime, TimePerTurn};

        use std::thread::sleep;
        use std::time::{Duration, Instant};

        use futures::{prelude::*, future::{self, ok, err, Either, FutureResult}};
        use tokio;
        use tokio::timer::Delay;

        let mut clock = ChessClock::new(
            2,
            BaseTime(Duration::new(2, 0)),
            TimePerTurn(Duration::new(2, 0)),
        );

        let when = Instant::now() + Duration::from_millis(2000);
        let task = Delay::new(when)
            .map_err(|_| ());
        let clocked_task = clock.bind(task)
            .then(|res| {
                match res {
                    Ok(Some(_)) => {
                        println!("Task succeeded");
                        ok(())
                    },
                    Ok(None) => {
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
