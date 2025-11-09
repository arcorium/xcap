use super::{
    dbus,
    impl_monitor::ImplMonitor,
    utils::{get_zbus_connection, get_zbus_portal_request, wait_zbus_response},
};
use crate::dir::{data_dir, project_dir};
use crate::platform::dbus::request::{
    on_blocking_response, request_handle_path, RequestProxyBlocking, Responses,
};
use crate::platform::dbus::screencast::{
    CreateSessionOption, CreateSessionResponse, CursorModes, PersistMode, ScreenCastProxyBlocking,
    SelectSourcesOption, SourceType, StartOption, StartResponse,
};
use crate::platform::dbus::session::session_handle_path;
use crate::platform::dbus::{generate_session_handle, generate_token_handle, screencast};
use crate::video_recorder::Condition;
use crate::{video_recorder::Frame, XCapError, XCapResult};
use bitflags::bitflags;
use log::{error, info, trace};
use pipewire::context::{ContextBox, ContextRc};
use pipewire::main_loop::MainLoopRc;
use pipewire::stream::{StreamBox, StreamRc};
use pipewire::{
    channel,
    context::Context,
    keys::{MEDIA_CATEGORY, MEDIA_ROLE, MEDIA_TYPE},
    main_loop::MainLoop,
    properties,
    spa::{
        param::{
            format::{FormatProperties, MediaSubtype, MediaType},
            format_utils,
            video::{VideoFormat, VideoInfoRaw},
            ParamType,
        },
        pod::{self, serialize::PodSerializer, Pod},
        utils::{Direction, Fraction, Rectangle, SpaTypes},
    },
    stream::{Stream, StreamFlags},
};
use std::borrow::Cow;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use std::{
    collections::HashMap,
    fmt,
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
};
use zbus::zvariant::OwnedValue;
use zbus::{
    blocking::Proxy,
    zvariant,
    zvariant::{DeserializeDict, OwnedFd, OwnedObjectPath, Type, Value},
};

const SCREEN_CAST_TOKEN_FILE_PATH: &str = "RESTORE_TOKEN";

bitflags! {
    #[derive(PartialEq, Ord, PartialOrd, Eq, Copy, Clone)]
    struct ScreenCastFlag : u8 {
        const HideCursor = 1;
        const EnableMulti = 2;
        const SavePermission = 4;  // Only available at minimum version 4
    }
}

/// https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
pub struct ScreenCast<'a> {
    proxy: ScreenCastProxyBlocking<'a>,
    flags: ScreenCastFlag,
    sources: SourceType,
    cursor_modes: CursorModes,
    restore_token: Option<String>,
}

impl ScreenCast<'_> {
    fn new(flags: ScreenCastFlag, sources: SourceType) -> XCapResult<Self> {
        let conn = get_zbus_connection();
        let proxy = ScreenCastProxyBlocking::new(&conn)?;

        let v = proxy.version()?;
        let modes = if flags.contains(ScreenCastFlag::HideCursor) {
            if v < 2 {
                return Err(XCapError::new(format!(
                    "Version {} does not have capability to fetch cursor modes",
                    v
                )));
            }

            let modes = proxy.available_cursor_modes()?;
            if !modes.is_hidden_available() {
                return Err(XCapError::new("Cursor hiding is not supported"));
            }
            modes
        } else {
            CursorModes::empty()
        };

        let restore_token = if flags.contains(ScreenCastFlag::SavePermission) {
            if v < 4 {
                return Err(XCapError::new(format!(
                    "Version {} does not have capability to save screen cast permission",
                    v
                )));
            }

            let path = data_dir().join(SCREEN_CAST_TOKEN_FILE_PATH);
            if path.is_file() {
                let mut result = String::new();
                let mut file = std::fs::File::open(path)?;
                file.read_to_string(&mut result)?;

                Some(result)
            } else {
                None
            }
        } else {
            None
        };

        Ok(ScreenCast {
            proxy,
            flags,
            sources,
            cursor_modes: modes,
            restore_token,
        })
    }

    pub fn create_session(&self) -> XCapResult<OwnedObjectPath> {
        let conn = get_zbus_connection();

        let handle_token = generate_token_handle();
        let session_handle_token = generate_session_handle();

        let resp: Responses<CreateSessionResponse> =
            on_blocking_response(conn, handle_token.as_str(), || {
                self.proxy.create_session(CreateSessionOption {
                    handle_token: &handle_token,
                    session_handle_token: &session_handle_token,
                })?;

                Ok(())
            })?;

        if !resp.is_success() {
            return Err(XCapError::new(format!(
                "got error response from portal: {}",
                resp.code
            )));
        }

        let session_path = session_handle_path(&conn, &session_handle_token)?;
        if session_path.as_str() != resp.session_handle.as_str() {
            return Err(XCapError::new("Session handle mismatch"));
        }

        Ok(session_path)
    }

    pub fn select_sources(&self, session: &OwnedObjectPath) -> XCapResult<()> {
        let conn = get_zbus_connection();
        let handle_token = generate_token_handle();

        let resp: Responses<()> = on_blocking_response(conn, handle_token.as_str(), || {
            let restore_token = if let Some(ref token) = self.restore_token
                && self.flags.contains(ScreenCastFlag::SavePermission)
            {
                Some(token.clone())
            } else {
                None
            };

            const PERSIST_MODE: PersistMode = PersistMode::None;
            self.proxy.select_sources(
                session.as_ref(),
                SelectSourcesOption {
                    handle_token: &handle_token,
                    types: self.sources,
                    multiple: self.flags.contains(ScreenCastFlag::EnableMulti),
                    cursor_mode: self
                        .cursor_modes
                        .best_mode(self.flags.contains(ScreenCastFlag::HideCursor)),
                    restore_token: restore_token.as_deref(),
                    persist_mode: PERSIST_MODE,
                },
            )?;

            Ok(())
        })?;

        if !resp.is_success() {
            return Err(XCapError::new(format!(
                "got error response from portal: {}",
                resp.code
            )));
        }

        Ok(())
    }

    pub fn start(
        &self,
        window_handle: Option<&str>,
        session: &OwnedObjectPath,
    ) -> XCapResult<screencast::StartResponse> {
        let conn = get_zbus_connection();
        let handle_token = generate_token_handle();

        let resp: Responses<StartResponse> =
            on_blocking_response(conn, handle_token.as_str(), || {
                self.proxy.start(
                    session.as_ref(),
                    window_handle.unwrap_or(""),
                    StartOption {
                        handle_token: &handle_token,
                    },
                )?;

                Ok(())
            })?;

        if !resp.is_success() {
            return Err(XCapError::new(format!(
                "got error response from portal: {}",
                resp.code
            )));
        }

        Ok(resp.results().clone())
    }
}

#[derive(Clone)]
pub struct WaylandVideoRecorder {
    #[allow(dead_code)]
    monitor: ImplMonitor,
    // sender: Sender<Frame>,
    condition: Arc<Mutex<Condition>>,
    condition_sender: channel::Sender<Condition>,
}

impl fmt::Debug for WaylandVideoRecorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaylandVideoRecorder")
            .field("monitor", &self.monitor)
            // .field("sender", &self.sender)
            .field("is_running", &self.condition)
            // Sender is not Debug
            // .field("control_tx", &self.control_tx)
            .finish()
    }
}

#[derive(Clone)]
struct ListenerUserData {
    pub format: VideoInfoRaw,
}

impl WaylandVideoRecorder {
    pub fn new(monitor: ImplMonitor) -> XCapResult<(Self, Receiver<Frame>)> {
        let (frame_sender, frame_receiver) = mpsc::channel();
        let (cond_sender, cond_receiver) = channel::channel();

        const FLAGS: ScreenCastFlag = ScreenCastFlag::empty();
        const SOURCES: SourceType = SourceType::Monitor;
        let screen_cast = ScreenCast::new(FLAGS, SOURCES)?;
        let session = screen_cast.create_session()?;
        screen_cast.select_sources(&session)?;
        let response = screen_cast.start(None, &session)?;

        // 获取流节点ID
        let stream_id = response
            .streams
            .ok_or(XCapError::new("Stream ID not found"))?
            .first()
            .ok_or(XCapError::new("Stream ID not found"))?
            .pipewire_node_id;

        let recorder = Self {
            monitor,
            // sender,
            condition: Arc::new(Mutex::new(Condition::Init)),
            condition_sender: cond_sender,
        };

        recorder.pipewire_capturer(stream_id, frame_sender, cond_receiver)?;

        Ok((recorder, frame_receiver))
    }

    pub fn pipewire_capturer(
        &self,
        stream_id: u32,
        sender: mpsc::Sender<Frame>,
        condition_receiver: channel::Receiver<Condition>,
    ) -> XCapResult<()> {
        let condition = self.condition.clone();

        pipewire::init();

        thread::spawn(move || {
            let main_loop = MainLoopRc::new(None)?;
            let context = ContextRc::new(&main_loop, None)?;
            let core = context.connect_rc(None)?;

            let user_data = ListenerUserData {
                format: Default::default(),
            };

            let stream = StreamRc::new(
                core,
                "XCap",
                properties::properties! {
                    *MEDIA_TYPE => "Video",
                    *MEDIA_CATEGORY => "Capture",
                    *MEDIA_ROLE => "Screen",
                },
            )?;

            let _listener = stream
                .add_local_listener_with_user_data(user_data)
                .param_changed(|_, user_data, id, param| {
                    let Some(param) = param else {
                        return;
                    };

                    if id != ParamType::Format.as_raw() {
                        return;
                    }

                    let (media_type, media_subtype) = match format_utils::parse_format(param) {
                        Ok(v) => v,
                        Err(err) => {
                            error!("Failed to parse format: {err:?}");
                            return;
                        }
                    };

                    if media_type != MediaType::Video || media_subtype != MediaSubtype::Raw {
                        return;
                    }

                    if let Err(err) = user_data.format.parse(param) {
                        error!("Failed to parse format: {err:?}");
                    }
                })
                .process(move |stream, user_data| {
                    let Ok(state) = condition.lock() else {
                        error!("Failed to lock is_running");
                        return;
                    };

                    match stream.dequeue_buffer() {
                        None => info!("stream.dequeue_buffer() returned None"),
                        Some(mut buffer) => {
                            let datas = buffer.datas_mut();
                            if datas.is_empty() {
                                return;
                            }
                            let size = user_data.format.size();

                            let Some(frame_data) = datas[0].data() else {
                                return;
                            };

                            let buffer = match user_data.format.format() {
                                VideoFormat::RGB => {
                                    let mut buf = vec![0; (size.width * size.height * 4) as usize];
                                    for (src, dst) in
                                        frame_data.chunks_exact(3).zip(buf.chunks_exact_mut(4))
                                    {
                                        dst[0] = src[0];
                                        dst[1] = src[1];
                                        dst[2] = src[2];
                                        dst[3] = 255;
                                    }

                                    buf
                                }
                                VideoFormat::RGBA => frame_data.to_vec(),
                                VideoFormat::RGBx => frame_data.to_vec(),
                                VideoFormat::BGRx => {
                                    let mut buf = frame_data.to_vec();
                                    for src in buf.chunks_exact_mut(4) {
                                        src.swap(0, 2);
                                    }

                                    buf
                                }
                                _ => {
                                    log::error!(
                                        "Unsupported format: {:?}",
                                        user_data.format.format()
                                    );
                                    return;
                                }
                            };

                            if state.is_running() {
                                let _ = sender.send(Frame::new(size.width, size.height, buffer));
                            }
                        }
                    }
                })
                .register()?;

            let obj = pod::object!(
                SpaTypes::ObjectParamFormat,
                ParamType::EnumFormat,
                pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
                pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
                pod::property!(
                    FormatProperties::VideoFormat,
                    Choice,
                    Enum,
                    Id,
                    VideoFormat::RGB,
                    VideoFormat::RGBA,
                    VideoFormat::RGBx,
                    VideoFormat::BGRx,
                    // VideoFormat::YUY2,
                    // VideoFormat::I420,
                ),
                pod::property!(
                    FormatProperties::VideoSize,
                    Choice,
                    Range,
                    Rectangle,
                    Rectangle {
                        width: 128,
                        height: 128
                    },
                    Rectangle {
                        width: 1,
                        height: 1
                    },
                    Rectangle {
                        width: 4096,
                        height: 4096
                    }
                ),
                pod::property!(
                    FormatProperties::VideoFramerate,
                    Choice,
                    Range,
                    Fraction,
                    Fraction { num: 24, denom: 1 },
                    Fraction { num: 0, denom: 1 },
                    Fraction {
                        num: 1000,
                        denom: 1
                    }
                ),
            );
            let values =
                PodSerializer::serialize(Cursor::new(Vec::new()), &pod::Value::Object(obj))
                    .map_err(XCapError::new)?
                    .0
                    .into_inner();

            let mut params =
                [Pod::from_bytes(&values).ok_or(XCapError::new("Failed to create Pod"))?];

            stream.connect(
                Direction::Input,
                Some(stream_id),
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
                &mut params,
            )?;

            // Used to pause/resume the stream
            let _attached = condition_receiver.attach(main_loop.loop_(), {
                let main_loop = main_loop.clone();
                move |cond| {
                    if let Err(e) = stream.set_active(cond.is_running()) {
                        error!("Failed to set stream active={}: {e:?}", cond.is_running());
                        return;
                    }

                    match cond {
                        Condition::Init => {} // when the pipewire already capturing but it is not started yet!
                        Condition::Running => {}
                        Condition::Paused => {
                            if let Err(e) = stream.flush(true) {
                                error!("Failed to flush: {e:?}");
                            }
                        }
                        Condition::Stopped => {
                            if let Err(e) = stream.flush(true) {
                                error!("Failed to flush: {e:?}");
                            }
                            main_loop.quit();
                        }
                    }
                }
            });

            main_loop.run();
            Result::<(), XCapError>::Ok(())
        });

        Ok(())
    }

    pub fn start(&self) -> XCapResult<()> {
        self.set_state(Condition::Running, Some(Condition::Stopped))
    }

    pub fn pause(&self) -> XCapResult<()> {
        self.set_state(Condition::Paused, Some(Condition::Stopped))
    }

    pub fn stop(&self) -> XCapResult<()> {
        self.set_state(Condition::Stopped, None)
    }

    fn set_state(
        &self,
        target_cond: Condition,
        disallowed_cond: Option<Condition>,
    ) -> XCapResult<()> {
        let Ok(mut cond) = self.condition.lock() else {
            error!("Failed to get condition lock");
            return Err(XCapError::new("Failed to get condition lock"));
        };
        if *cond == target_cond {
            trace!("Already in state: {:?}", target_cond);
            return Ok(());
        }
        if let Some(disallowed_cond) = disallowed_cond
            && *cond == disallowed_cond
        {
            error!(
                "cannot set state from {} to {} state",
                *cond, disallowed_cond
            );
            return Err(XCapError::new(format!(
                "cannot set state from {} to {} state",
                *cond, disallowed_cond
            )));
        }
        trace!("set state: {}", target_cond);
        *cond = target_cond;
        let _ = self.condition_sender.send(target_cond);
        Ok(())
    }
}

impl Drop for WaylandVideoRecorder {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            error!("Failed to stop wayland video recorder: {e:?}");
        }
    }
}
