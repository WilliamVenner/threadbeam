use super::{ThreadBeamFlags, ThreadBeamRx, ThreadBeamState, ThreadBeamTx};
use alloc::boxed::Box;
use core::{mem::MaybeUninit, ptr::NonNull};
use spin::Mutex;

pub(super) struct ThreadBeamInner<T> {
	lock: Mutex<ThreadBeamState<T>>,
}

impl<T: Send> ThreadBeamTx<T> {
	/// Send a value to the receiving side of the thread beam.
	pub fn send(self, value: T) {
		let inner = unsafe { self.0.as_ref() };

		let mut lock = inner.lock.lock();
		lock.set_data(value);
	}
}
impl<T: Send> Drop for ThreadBeamTx<T> {
	fn drop(&mut self) {
		let deallocate = {
			let inner = unsafe { self.0.as_ref() };
			inner.lock.lock().drop_tx()
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
		loop {
			let mut lock = inner.lock.lock();
			if lock.has_data() {
				lock.flags &= !ThreadBeamFlags::HAS_DATA;
				return Some(unsafe { lock.data.assume_init_read() });
			} else if lock.hung_up() {
				return None;
			} else {
				drop(lock);
				core::hint::spin_loop();
				continue;
			}
		}
	}
}
impl<T: Send> Drop for ThreadBeamRx<T> {
	fn drop(&mut self) {
		let deallocate = {
			let inner = unsafe { self.0.as_ref() };
			inner.lock.lock().drop_rx()
		};
		if deallocate {
			unsafe { Box::from_raw(self.0.as_ptr()) };
		}
	}
}

/// Creates a new thread beam channel pair.
///
/// # Example
///
/// ```rust
/// let (tx, rx) = threadbeam::channel();
///
/// std::thread::spawn(move || {
///     tx.send(String::from("Hello, world!"));
/// });
///
/// let hello = rx.recv();
/// assert_eq!(hello.as_deref(), Some("Hello, world!"));
/// ```
pub fn channel<T: Send>() -> (ThreadBeamTx<T>, ThreadBeamRx<T>) {
	let inner = Box::into_raw(Box::new(ThreadBeamInner {
		lock: Mutex::new(ThreadBeamState {
			data: MaybeUninit::uninit(),
			flags: ThreadBeamFlags::TX | ThreadBeamFlags::RX,
		}),
	}));
	let inner = unsafe { NonNull::new_unchecked(inner) };
	(ThreadBeamTx(inner), ThreadBeamRx(inner))
}
