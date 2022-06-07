use super::{ThreadBeamFlags, ThreadBeamRx, ThreadBeamState, ThreadBeamTx};
use core::{mem::MaybeUninit, ptr::NonNull};

#[cfg(feature = "parking_lot")]
use parking_lot::{Condvar, Mutex};

#[cfg(not(feature = "parking_lot"))]
use std::sync::{Condvar, Mutex};

#[cfg(not(feature = "parking_lot"))]
macro_rules! cvar_wait {
	($lock:ident = $cvar:expr) => {
		$lock = $cvar.wait($lock).unwrap();
	};
}
#[cfg(feature = "parking_lot")]
macro_rules! cvar_wait {
	($lock:ident = $cvar:expr) => {
		$cvar.wait(&mut $lock);
	};
}

#[cfg(not(feature = "parking_lot"))]
macro_rules! lock_mutex {
	($mutex:expr) => {
		$mutex.lock().unwrap()
	};
}
#[cfg(feature = "parking_lot")]
macro_rules! lock_mutex {
	($mutex:expr) => {
		$mutex.lock()
	};
}

pub(super) struct ThreadBeamInner<T> {
	lock: Mutex<ThreadBeamState<T>>,
	cvar: Condvar,
}

impl<T: Send> ThreadBeamTx<T> {
	/// Send a value to the receiving side of the thread beam.
	pub fn send(self, value: T) {
		let inner = unsafe { self.0.as_ref() };

		let mut lock = lock_mutex!(inner.lock);
		lock.set_data(value);

		inner.cvar.notify_all();
	}
}
impl<T: Send> Drop for ThreadBeamTx<T> {
	fn drop(&mut self) {
		let deallocate = {
			let inner = unsafe { self.0.as_ref() };

			let mut lock = lock_mutex!(inner.lock);
			let deallocate = lock.drop_tx();

			inner.cvar.notify_all();

			deallocate
		};
		if deallocate {
			unsafe { Box::from_raw(self.0.as_ptr()) };
		}
	}
}

impl<T: Send> ThreadBeamRx<T> {
	/// Receive the value sent by the sending side of the thread beam.
	///
	/// Returns `None` if the sending side of the thread beam has been dropped.
	pub fn recv(self) -> Option<T> {
		let inner = unsafe { self.0.as_ref() };

		let mut lock = lock_mutex!(inner.lock);

		if lock.has_data() {
			lock.flags &= !ThreadBeamFlags::HAS_DATA;
			return Some(unsafe { lock.data.assume_init_read() });
		} else if lock.hung_up() {
			return None;
		}

		cvar_wait!(lock = inner.cvar);

		if lock.has_data() {
			lock.flags &= !ThreadBeamFlags::HAS_DATA;
			Some(unsafe { lock.data.assume_init_read() })
		} else {
			None
		}
	}
}
impl<T: Send> Drop for ThreadBeamRx<T> {
	fn drop(&mut self) {
		let deallocate = {
			let inner = unsafe { self.0.as_ref() };
			lock_mutex!(inner.lock).drop_rx()
		};
		if deallocate {
			unsafe { Box::from_raw(self.0.as_ptr()) };
		}
	}
}

/// Creates a new thread beam channel pair.
///
/// Also see [spawn] for a more convenient way to spawn a thread with a thread beam.
///
/// # Example
///
/// ```rust
/// let (tx, rx) = threadbeam::channel();
///
/// # let j =
/// std::thread::spawn(move || {
///     tx.send(String::from("Hello, world!"));
/// });
///
/// let hello = rx.recv();
/// assert_eq!(hello.as_deref(), Some("Hello, world!"));
///
/// # j.join().unwrap();
/// ```
pub fn channel<T: Send>() -> (ThreadBeamTx<T>, ThreadBeamRx<T>) {
	let inner = Box::into_raw(Box::new(ThreadBeamInner {
		lock: Mutex::new(ThreadBeamState {
			data: MaybeUninit::uninit(),
			flags: ThreadBeamFlags::TX | ThreadBeamFlags::RX,
		}),
		cvar: Condvar::new(),
	}));
	let inner = unsafe { NonNull::new_unchecked(inner) };
	(ThreadBeamTx(inner), ThreadBeamRx(inner))
}

#[inline]
/// Helper for spawning a new thread with a beam.
///
/// # Example
///
/// ```rust
/// let (hello, thread) = threadbeam::spawn(move |tx| {
///     tx.send(String::from("Hello, world!"));
///     // your code...
///     String::from("Thread completed!")
/// });
///
/// assert_eq!(hello.as_deref(), Some("Hello, world!"));
/// assert_eq!(thread.join().ok().as_deref(), Some("Thread completed!"));
/// ```
pub fn spawn<T, R, F>(spawn: F) -> (Option<T>, std::thread::JoinHandle<R>)
where
	F: FnOnce(ThreadBeamTx<T>) -> R,
	F: Send + 'static,
	T: Send + 'static,
	R: Send + 'static,
{
	let (tx, rx) = channel();
	let join = std::thread::spawn(move || spawn(tx));
	(rx.recv(), join)
}
