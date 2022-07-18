use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::panic::{AssertUnwindSafe, UnwindSafe};

use super::{AsyncDrop, AsyncDropGuard};

/// SyncDrop wraps an [AsyncDropGuard] and calls `AsyncDropGuard::async_drop` on it in its
/// synchronous [Drop] destructor.
///
/// WARNING: This can cause deadlocks, see https://stackoverflow.com/questions/71541765/rust-async-drop
pub struct SyncDrop<T: Debug + AsyncDrop>(Option<AsyncDropGuard<T>>);

impl<T: Debug + AsyncDrop> SyncDrop<T> {
    pub fn new(v: AsyncDropGuard<T>) -> Self {
        Self(Some(v))
    }

    pub fn into_inner_dont_drop(mut self) -> AsyncDropGuard<T> {
        self.0.take().expect("Already dropped")
    }

    pub fn inner(&self) -> &AsyncDropGuard<T> {
        self.0.as_ref().expect("Already dropped")
    }
}

impl<T: Debug + AsyncDrop> Drop for SyncDrop<T> {
    fn drop(&mut self) {
        if let Some(mut v) = self.0.take() {
            if std::thread::panicking() {
                // v is unwind safe because we're destroying it and not using it anymore after the async_drop call
                // TODO Is this actually true? What if its Drop implementation relies on state?
                let mut v = AssertUnwindSafe(v);
                // If we're dropping this because of a panic, we want to avoid causing a double
                // panic because they don't show any error message or backtrace and are super hard
                // to debug. Instead, log the inner panic and let the outer panic continue.
                if let Err(panic) = std::panic::catch_unwind(move || {
                    futures::executor::block_on(v.async_drop()).unwrap()
                }) {
                    log::error!("Double panic.\nInner panic: {:?}", panic);
                }
            } else {
                futures::executor::block_on(v.async_drop()).unwrap()
            }
        }
    }
}

impl<T: Debug + AsyncDrop + UnwindSafe> Deref for SyncDrop<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_ref().expect("Already dropped")
    }
}

impl<T: Debug + AsyncDrop + UnwindSafe> DerefMut for SyncDrop<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.as_mut().expect("Already dropped")
    }
}
