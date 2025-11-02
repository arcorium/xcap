use std::fmt::Display;
use std::sync::{Condvar, Mutex};

use crate::{XCapResult, platform::impl_video_recorder::ImplVideoRecorder};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Condition {
    Init, // Used when the recorder is created and not started yet, to differentiate from the Stopped state.
    Running,
    Paused,
    Stopped,
}

impl Condition {
    pub fn is_running(&self) -> bool {
        *self == Condition::Running
    }
}

impl Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Condition::Init => write!(f, "Init"),
            Condition::Running => write!(f, "Running"),
            Condition::Paused => write!(f, "Paused"),
            Condition::Stopped => write!(f, "Stopped"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub raw: Vec<u8>,
}

impl Frame {
    pub fn new(width: u32, height: u32, raw: Vec<u8>) -> Self {
        Self { width, height, raw }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct RecorderWaker {
    parking: Mutex<bool>,
    condvar: Condvar,
}

impl RecorderWaker {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            parking: Mutex::new(true),
            condvar: Condvar::new(),
        }
    }

    /// Wakes up one thread waiting on the condition variable associated with the parking status.
    ///
    /// This method sets the internal `parking` flag to `false` and notifies
    /// one thread that is currently waiting on the condition variable (`condvar`). This allows
    /// a thread that was waiting to proceed. The `wake` function is thread-safe and acquires
    /// a mutex lock to modify the shared `parking` state.
    ///
    /// # Returns
    ///
    /// Returns an `XCapResult<()>`, which is `Ok(())` if the operation is successful, or an error
    /// if locking the internal mutex fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if acquiring the lock on `self.parking` fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// let my_struct = MyStruct::new();
    /// my_struct.wake()?;
    /// ```
    ///
    /// In this example, the `wake` function is called to notify one waiting thread
    /// that it can proceed by waking it, provided there are no errors.
    ///
    /// # Notes
    ///
    /// - Ensure that other threads are using the same condition variable to wait.
    /// - If no threads are waiting on the condition variable, calling `wake` has no effect.
    #[allow(dead_code)]
    pub fn wake(&self) -> XCapResult<()> {
        let mut parking = self.parking.lock()?;
        *parking = false;
        self.condvar.notify_one();

        Ok(())
    }

    ///
    /// Puts the current instance into a "sleep" or "on park" state by modifying its internal state.
    ///
    /// This method acquires a lock on the `parking` field (a mutex-protected value),
    /// sets its value to `true` to indicate the "sleep" state, and then releases the lock.
    ///
    /// # Returns
    ///
    /// - `XCapResult<()>`: Returns an `Ok(())` on success, or an error if the lock on the `parking` field cannot be acquired.
    ///
    /// # Errors
    ///
    /// This function may return an error of type `XCapResult::Err` if acquiring the lock on
    /// the `parking` field fails.
    ///
    /// # Example
    ///
    /// ```
    /// let instance = RecorderWaker::new();
    /// instance.sleep()?;
    /// ```
    ///
    /// ```
    #[allow(dead_code)]
    pub fn sleep(&self) -> XCapResult<()> {
        let mut parking = self.parking.lock()?;
        *parking = true;

        Ok(())
    }

    /// Waits until the condition variable is notified and the internal parking flag is `false`.
    ///
    /// This function locks the `parking` mutex and repeatedly checks if the parking flag is `false`.
    /// If the flag is `true`, the function waits on the condition variable, releasing the lock and blocking
    /// the current thread until a notification is received. Once the condition variable is notified and the flag
    /// becomes `false`, the function returns successfully.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the thread successfully waits until the condition is met.
    /// * `Err(XCapError)` - If an error occurs while acquiring the lock or waiting on the condition variable.
    ///
    /// # Errors
    ///
    /// This function can fail if there is an issue with acquiring the lock on the `parking` mutex or
    /// waiting on the condition variable. In such cases, the error will be propagated as an `XCapError`.
    ///
    /// # Notes
    ///
    /// - This function uses a loop to guarantee that spurious wakeups do not break the logic.
    /// - The function assumes that the state is coordinated externally and will eventually set the `parking` flag to `false`
    /// so that the waiting can stop.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let resource = RecorderWaker::new();
    /// // Assuming `resource.wait()` is safe to call
    /// match resource.wait() {
    ///     Ok(_) => println!("Condition met, and the resource is accessible."),
    ///     Err(e) => eprintln!("Error while waiting: {:?}", e),
    /// }
    /// ```
    #[allow(dead_code)]
    pub fn wait(&self) -> XCapResult<()> {
        let mut parking = self.parking.lock()?;
        while *parking {
            parking = self.condvar.wait(parking)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct VideoRecorder {
    impl_video_recorder: ImplVideoRecorder,
}

impl VideoRecorder {
    pub(crate) fn new(impl_video_recorder: ImplVideoRecorder) -> VideoRecorder {
        VideoRecorder {
            impl_video_recorder,
        }
    }
}

impl VideoRecorder {
    pub fn start(&self) -> XCapResult<()> {
        self.impl_video_recorder.start()
    }
    pub fn pause(&self) -> XCapResult<()> {
        self.impl_video_recorder.pause()
    }
    pub fn stop(&self) -> XCapResult<()> {
        self.impl_video_recorder.stop()
    }
}
