use crate::video_recorder::Condition;
use crate::XCapResult;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

struct Frame {
    inner: crate::Frame,
    number: u32,
}

struct ScreenCopyManager {
    inner: ZwlrScreencopyManagerV1,
}

impl ScreenCopyManager {
    fn new() -> Self {}

    fn record(&self) -> XCapResult<()> {
        self.inner.capture_output()
        Ok(())
    }

    fn screenshot(&self) -> XCapResult<Frame> {

    }

    fn screenshot_region(&self, region: (i32, i32)) -> XCapResult<Frame> {

    }
}

#[derive(Clone)]
pub struct WlrootVideoRecorder {
    condition: Condition,
}

impl WlrootVideoRecorder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start(&self) -> XCapResult<()> {
        Ok(())
    }

    pub fn pause(&self) -> XCapResult<()> {
        Ok(())
    }

    pub fn stop(&self) -> XCapResult<()> {
        Ok(())
    }
}
