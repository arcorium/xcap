use super::impl_monitor::ImplMonitor;
use crate::error::{XCapError, XCapResult};
use crate::video_recorder::{Condition, Frame, RecorderWaker};
use log::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct XorgVideoRecorder {
    monitor: ImplMonitor,
    condition: Arc<Mutex<Condition>>,
    recorder_waker: Arc<RecorderWaker>,
}

impl XorgVideoRecorder {
    pub fn new(monitor: ImplMonitor) -> XCapResult<(Self, Receiver<Frame>)> {
        let (sender, receiver) = mpsc::channel();
        let recorder = Self {
            monitor,
            condition: Arc::new(Mutex::new(Condition::Init)),
            recorder_waker: Arc::new(RecorderWaker::new()),
        };

        recorder.on_frame(sender)?;

        Ok((recorder, receiver))
    }

    pub fn on_frame(&self, sender: Sender<Frame>) -> XCapResult<()> {
        let monitor = self.monitor.clone();
        let cond = self.condition.clone();
        let recorder_waker = self.recorder_waker.clone();

        thread::spawn(move || {
            loop {
                if let Err(err) = recorder_waker.wait() {
                    error!("Recorder waker error: {err:?}");
                    break Err(err);
                }

                let cond = match cond.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        error!("Failed to lock running flag: {e:?}");
                        break Err(XCapError::from(e));
                    }
                };

                if !cond.is_running() {
                    // when condition is Condition::Stopped and the waker is woken up it will make the spawn
                    // to quit
                    drop(sender);
                    break Ok(());
                }

                match monitor.capture_image() {
                    Ok(image) => {
                        let width = image.width();
                        let height = image.height();
                        let raw = image.into_raw();

                        let frame = Frame::new(width, height, raw);
                        if let Err(e) = sender.send(frame) {
                            error!("Failed to send frame: {e:?}");
                            break Err(XCapError::new(format!("Failed to send frame: {e}")));
                        }
                    }
                    Err(e) => {
                        error!("Failed to capture frame: {e:?}");
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                }

                thread::sleep(Duration::from_millis(1)); // TODO: Add fps capability
            }
        });

        Ok(())
    }

    pub fn start(&self) -> XCapResult<()> {
        let mut running = self.condition.lock().map_err(XCapError::from)?;
        match *running {
            Condition::Running => {
                return Ok(());
            }
            Condition::Stopped => {
                return Err(XCapError::new("Recorder is already stopped"));
            }
            _ => {}
        }
        *running = Condition::Running;

        self.recorder_waker.wake()?;

        Ok(())
    }

    pub fn pause(&self) -> XCapResult<()> {
        let mut running = self.condition.lock().map_err(XCapError::from)?;
        match *running {
            Condition::Paused => {
                return Ok(());
            }
            Condition::Stopped => {
                return Err(XCapError::new("Recorder is already stopped"));
            }
            _ => {}
        }
        *running = Condition::Paused;

        self.recorder_waker.sleep()?;

        Ok(())
    }

    pub fn stop(&self) -> XCapResult<()> {
        let mut running = self.condition.lock().map_err(XCapError::from)?;
        if *running == Condition::Stopped {
            return Ok(());
        }
        *running = Condition::Stopped;

        self.recorder_waker.wake()?;
        Ok(())
    }
}

impl Drop for XorgVideoRecorder {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            error!("Failed to stop recorder: {e:?}");
        }
    }
}
