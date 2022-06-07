//! # threadbeam
//!
//! A simple, specialized channel type for beaming data out of a newly spawned thread.
//!
//! # Usage
//!
//! First, add `threadbeam` to your crate's dependencies in Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! threadbeam = "0"
//! ```
//!
//! ## Examples
//!
//! ```rust
//! let (tx, rx) = threadbeam::channel();
//!
//! # let j =
//! std::thread::spawn(move || {
//!     tx.send(String::from("Hello, world!"));
//! });
//!
//! let hello = rx.recv();
//! assert_eq!(hello.as_deref(), Some("Hello, world!"));
//!
//! # j.join().unwrap();
//! ```
//!
//! ```rust
//! # #[cfg(not(feature = "no_std"))]
//! let (hello, thread) = threadbeam::spawn(move |tx| {
//!     tx.send(String::from("Hello, world!"));
//!     // your code...
//!     String::from("Thread completed!")
//! });
//!
//! # #[cfg(not(feature = "no_std"))]
//! assert_eq!(hello.as_deref(), Some("Hello, world!"));
//! # #[cfg(not(feature = "no_std"))]
//! assert_eq!(thread.join().ok().as_deref(), Some("Thread completed!"));
//! ```
//!
//! ## `parking_lot`
//!
//! To use [`parking_lot`](https://docs.rs/parking_lot/latest) instead of the standard library's implementations of `Condvar` and `Mutex`, enable the `parking_lot` feature in your Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! threadbeam = { version = "0", features = ["parking_lot"] }
//! ```
//!
//! ## `no_std` via `spin`
//!
//! For `no_std` environments, enable the `no_std` feature in your Cargo.toml:
//!
//! This will use [`spin`](https://docs.rs/spin/latest) as the provider of the `Mutex` implementation.
//!
//! ```toml
//! [dependencies]
//! threadbeam = { version = "0", features = ["no_std"] }
//! ```

#![cfg_attr(all(feature = "no_std", not(test)), no_std)]
#![deny(missing_docs)]
#![deny(clippy::tabs_in_doc_comments)]

#[cfg(all(feature = "no_std", feature = "parking_lot"))]
compile_error!("Cannot use `parking_lot` feature with `no_std` feature");

#[cfg(feature = "no_std")]
extern crate alloc;
#[cfg(all(test, feature = "no_std"))]
use alloc::string::String;

#[cfg(feature = "no_std")]
mod no_std;
#[cfg(feature = "no_std")]
use no_std as r#impl;

#[cfg(not(feature = "no_std"))]
mod r#impl;

use r#impl::ThreadBeamInner;
pub use r#impl::*;

use core::{mem::MaybeUninit, ptr::NonNull};

/// The sending side of a thread beam.
pub struct ThreadBeamTx<T: Send>(NonNull<ThreadBeamInner<T>>);

/// The receiving side of a thread beam.
pub struct ThreadBeamRx<T: Send>(NonNull<ThreadBeamInner<T>>);

unsafe impl<T: Send> Sync for ThreadBeamTx<T> {}
unsafe impl<T: Send> Send for ThreadBeamTx<T> {}

unsafe impl<T: Send> Sync for ThreadBeamRx<T> {}
unsafe impl<T: Send> Send for ThreadBeamRx<T> {}

bitflags::bitflags! {
	struct ThreadBeamFlags: u8 {
		// Option<T> but packed into a bitflag
		const HAS_DATA = 0b10000000;

		// Whether the sending side is closed
		const TX = 0b01000000;

		// Whether the receiving side is closed
		const RX = 0b00100000;
	}
}
struct ThreadBeamState<T> {
	data: MaybeUninit<T>,
	flags: ThreadBeamFlags,
}
impl<T> ThreadBeamState<T> {
	#[inline(always)]
	pub fn set_data(&mut self, value: T) {
		debug_assert!(!self.has_data());
		self.flags |= ThreadBeamFlags::HAS_DATA;
		self.data = MaybeUninit::new(value);
	}

	#[inline(always)]
	pub fn has_data(&self) -> bool {
		self.flags & ThreadBeamFlags::HAS_DATA != ThreadBeamFlags::empty()
	}

	#[inline(always)]
	pub fn hung_up(&self) -> bool {
		self.flags & (ThreadBeamFlags::TX | ThreadBeamFlags::RX) != (ThreadBeamFlags::TX | ThreadBeamFlags::RX)
	}

	#[must_use]
	#[inline(always)]
	pub fn drop_tx(&mut self) -> bool {
		self.flags &= !ThreadBeamFlags::TX;
		self.flags & ThreadBeamFlags::RX == ThreadBeamFlags::empty()
	}

	#[must_use]
	#[inline(always)]
	pub fn drop_rx(&mut self) -> bool {
		self.flags &= !ThreadBeamFlags::RX;
		self.flags & ThreadBeamFlags::TX == ThreadBeamFlags::empty()
	}
}
impl<T> Drop for ThreadBeamState<T> {
	#[inline(always)]
	fn drop(&mut self) {
		if self.has_data() {
			self.flags &= !ThreadBeamFlags::HAS_DATA;
			unsafe { core::ptr::drop_in_place(self.data.as_mut_ptr()) };
			self.data = MaybeUninit::uninit();
		}
	}
}

#[test]
fn test_thread_beam() {
	let (tx, rx) = channel::<String>();

	let t = std::thread::spawn(move || {
		tx.send(String::from("Hello, world!"));
	});

	assert_eq!(rx.recv().as_deref(), Some("Hello, world!"));
	t.join().unwrap();
}

#[test]
fn test_dropped_thread_beam() {
	let (tx, rx) = channel::<String>();
	drop(tx);
	assert_eq!(rx.recv(), None);
}

#[test]
fn test_dropped_thread_beam_2() {
	let (tx, rx) = channel::<String>();

	let t = std::thread::spawn(move || {
		std::thread::sleep(std::time::Duration::from_secs(1));
		drop(tx);
	});

	assert_eq!(rx.recv(), None);
	t.join().unwrap();
}

#[test]
fn test_dropped_thread_beam_3() {
	let (tx, rx) = channel::<String>();

	let t = std::thread::spawn(move || {
		drop(tx);
	});

	assert_eq!(rx.recv(), None);
	t.join().unwrap();
}

#[test]
fn test_weird_usage() {
	let (tx, rx) = channel::<String>();
	tx.send(String::from("Hello, world!"));
	assert_eq!(rx.recv().as_deref(), Some("Hello, world!"));
}

#[test]
fn test_weird_usage_2() {
	let (tx, rx) = channel::<String>();
	let t = std::thread::spawn(move || {
		assert_eq!(rx.recv().as_deref(), Some("Hello, world!"));
	});
	tx.send(String::from("Hello, world!"));
	t.join().unwrap();
}

#[test]
fn test_never_recv() {
	let (tx, rx) = channel::<String>();
	tx.send(String::from("Hello, world!"));
	drop(rx);
}

#[test]
fn test_drop_rx_then_send() {
	let (tx, rx) = channel::<String>();
	drop(rx);
	tx.send(String::from("Hello, world!"));
}
