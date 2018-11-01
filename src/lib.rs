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
//!   let when = Instant::now() + Duration::from_secs(2);
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
//!
//! # Implementation details
//!
//! `ChessClock` is [ARCed][std::sync::Arc] and (rw-mutexed)[std::sync::RwLock] so that it works
//! nicely with the borrow checker, but by nature of its design, which is sequential passing of
//! turns, it shouldn't be used simultaneously by two threads.
//!
//! It tracks the time of each player, a single global time-per-turn, the active player index, and
//! the time the last turn was passed, so it can calculate the time to subtract from the next
//! active player's clock.

#![feature(duration_as_u128)]

extern crate futures;
extern crate tokio;

mod chess_clock;
pub use chess_clock::{BaseTime, ChessClock, ClockedFuture, TimePerTurn};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        use super::{BaseTime, ChessClock, TimePerTurn};

        use std::thread::sleep;
        use std::time::{Duration, Instant};

        use futures::{
            future::{self, err, ok, Either, FutureResult},
            prelude::*,
        };
        use tokio;
        use tokio::timer::Delay;

        let bt = 4;
        let tpt = 1;
        let n_players = 2;
        println!("Time control (seconds): {};{}", bt, tpt);
        println!("Players: {}", n_players);
        let mut clock_master = ChessClock::new(
            n_players,
            BaseTime(Duration::new(4, 0)),
            TimePerTurn(Duration::new(1, 0)),
        );

        for i in 0..10 {
            let clock = clock_master.clone();
            let turn_length = 2;
            let player_num = i % n_players + 1;
            println!("---------------------------------------");
            println!(
                "Player {} time remaining: {}",
                player_num,
                clock.active_player_time_remaining().as_millis()
            );
            println!("Player {} turn length: {}", player_num, turn_length * 1000);
            let when = Instant::now() + Duration::from_secs(turn_length);
            let task = Delay::new(when).map_err(|_| ());
            let clocked_task = clock.bind(task).then(move |res| match res {
                Ok(Some(_)) => {
                    println!("Player {} successfully took their turn.", player_num);
                    ok(())
                }
                Ok(None) => {
                    println!("Player {} timed out", player_num);
                    ok(())
                }
                _ => err(()),
            });
            tokio::run(clocked_task);
        }
    }
}
