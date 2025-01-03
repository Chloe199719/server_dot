#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::print_stdout,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::integer_division
)]
pub mod game_state;
pub mod packet;
pub mod server;
pub mod tasks;
pub mod telemetry;
