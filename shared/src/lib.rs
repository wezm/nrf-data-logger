// make `std` available when testing
#![cfg_attr(not(test), no_std)]

pub mod govee;
pub mod bluetooth;
