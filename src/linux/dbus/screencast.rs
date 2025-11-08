use crate::platform::dbus::request::FromResponse;
use crate::{XCapError, XCapResult};
use bitflags::bitflags;
use std::collections::HashMap;
use std::ops::BitAnd;
use zbus::zvariant;
use zbus::zvariant::Value::Value;
use zbus::zvariant::{ObjectPath, OwnedValue, Signature};

#[derive(zvariant::SerializeDict, zvariant::Type, Debug)]
#[zvariant(signature = "a{sv}")]
pub struct CreateSessionOption<'a> {
    pub handle_token: &'a str,
    pub session_handle_token: &'a str,
}

#[derive(Debug)]
pub struct CreateSessionResponse<'a> {
    pub session_handle: ObjectPath<'a>,
}

impl CreateSessionResponse<'_> {
    const KEYS: [&'static str; 1] = ["session_handle"];
}

impl<'a> FromResponse for CreateSessionResponse<'a> {
    fn try_from_response(map: &mut HashMap<String, OwnedValue>) -> XCapResult<Self> {
        Ok(Self {
            session_handle: map
                .remove(Self::KEYS[0])
                .ok_or_else(|| XCapError::new(format!("map has no key {}", Self::KEYS[0])))?
                .try_into()?,
        })
    }
}

#[derive(Debug, zvariant::Type, zvariant::DeserializeDict, zvariant::Value)]
#[zvariant(signature = "a{sv}")]
pub struct StartStreamProperty {
    pub id: Option<String>,
    pub position: Option<(i32, i32)>,
    pub size: Option<(i32, i32)>,
    pub source_type: u32, // TODO: Change into SourceType
    pub mapping_id: Option<String>,
}

impl StartStreamProperty {
    const KEYS: [&'static str; 5] = ["id", "position", "size", "source_type", "mapping_id"];
}

impl FromResponse for StartStreamProperty {
    fn try_from_response(map: &mut HashMap<String, OwnedValue>) -> XCapResult<Self> {
        Ok(Self {
            id: map.remove(Self::KEYS[0]).and_then(|v| v.try_into().ok()),
            position: map.remove(Self::KEYS[1]).and_then(|v| v.try_into().ok()),
            size: map.remove(Self::KEYS[2]).and_then(|v| v.try_into().ok()),
            source_type: map
                .remove(Self::KEYS[3])
                .ok_or_else(|| XCapError::new(format!("map has no key {}", Self::KEYS[3])))?
                .try_into()
                .map_err(XCapError::new)?,
            mapping_id: map.remove(Self::KEYS[4]).and_then(|v| v.try_into().ok()),
        })
    }
}

#[derive(Debug, zvariant::Type, serde::Deserialize, zvariant::Value)]
#[zvariant(signature = "ua{sv}")]
pub struct StartStreamResponse {
    pub pipewire_node_id: u32,
    pub property: StartStreamProperty,
}

#[derive(Debug, zvariant::Type, serde::Deserialize)]
#[zvariant(signature = "a{sv}")]
pub struct StartResponse {
    pub streams: Vec<StartStreamResponse>,
    pub restore_token: Option<String>,
}

impl StartResponse {
    const KEYS: [&'static str; 2] = ["streams", "restore_token"];
}

impl FromResponse for StartResponse {
    fn try_from_response(map: &mut HashMap<String, OwnedValue>) -> XCapResult<Self> {
        Ok(Self {
            streams: map
                .remove(Self::KEYS[0])
                .ok_or_else(|| XCapError::new(format!("map has no key {}", Self::KEYS[0])))?
                .try_into()?,
            restore_token: map.remove(Self::KEYS[1]).and_then(|v| v.try_into().ok()),
        })
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Ord, PartialOrd, Eq, Copy, Clone, serde::Deserialize, serde::Serialize)]
    pub struct CursorModes : u32 {
        const Hidden = 1 << 0;
        const Embedded = 1 << 1;
        const Metadata= 1 << 2;
    }

    #[derive(Debug, PartialEq, Ord, PartialOrd, Eq, Copy, Clone, serde::Deserialize, serde::Serialize)]
    pub struct SourceType: u32 {
        const Monitor = 1 << 0;
        const Window = 1 << 1;
        const Virtual = 1 << 2;
    }
}

impl zvariant::Type for CursorModes {
    const SIGNATURE: &'static zvariant::Signature = &zvariant::Signature::U32;
}

impl From<zvariant::OwnedValue> for CursorModes {
    fn from(value: zvariant::OwnedValue) -> Self {
        match value.downcast_ref::<u32>() {
            Ok(v) => Self::from_bits_truncate(v),
            Err(_) => Self::empty(),
        }
    }
}

impl zvariant::Type for SourceType {
    const SIGNATURE: &'static zvariant::Signature = &zvariant::Signature::U32;
}

impl From<zvariant::OwnedValue> for SourceType {
    fn from(value: zvariant::OwnedValue) -> Self {
        match value.downcast_ref::<u32>() {
            Ok(v) => Self::from_bits_truncate(v),
            Err(_) => Self::empty(),
        }
    }
}

impl CursorModes {
    pub fn is_hidden_available(&self) -> bool {
        const EXPECTED: CursorModes = CursorModes::Hidden.union(CursorModes::Metadata);
        self.bitand(EXPECTED) > CursorModes::Hidden
    }

    pub fn best_mode(&self, show: bool) -> Self {
        if show {
            if self.contains(CursorModes::Embedded) {
                return CursorModes::Embedded;
            }
            if self.contains(CursorModes::Metadata) {
                return CursorModes::Metadata;
            }
        }

        if self.contains(CursorModes::Hidden) {
            CursorModes::Hidden
        } else {
            CursorModes::Metadata
        }
    }
}

#[derive(Debug, zvariant::Type, serde::Deserialize, serde::Serialize)]
#[repr(u32)]
pub enum PersistMode {
    None,
    Session,
    System,
}

#[derive(Debug, zvariant::Type, zvariant::SerializeDict)]
#[zvariant(signature = "a{sv}")]
pub struct SelectSourcesOptionMap<'a> {
    pub handle_token: &'a str,
    pub types: SourceType,
    pub multiple: bool,
    pub cursor_mode: CursorModes,
    pub restore_token: Option<String>,
    pub persist_mode: PersistMode,
}

#[derive(Debug, zvariant::Type, serde::Serialize)]
pub struct SelectSourcesOptions<'a> {
    pub session_handle: zvariant::ObjectPath<'a>,
    pub options: SelectSourcesOptionMap<'a>,
}

#[derive(Debug, zvariant::Type, zvariant::SerializeDict)]
#[zvariant(signature = "a{sv}")]
pub struct StartOptionMap<'a> {
    pub handle_token: &'a str,
}

#[derive(Debug, zvariant::Type, serde::Serialize)]
pub struct StartOptions<'a> {
    pub session_handle: zvariant::ObjectPath<'a>,
    pub parent_window: &'a str,
    pub options: StartOptionMap<'a>,
}

#[zbus::proxy(
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop",
    interface = "org.freedesktop.portal.ScreenCast"
)]
pub trait ScreenCast {
    fn create_session(
        &self,
        options: CreateSessionOption<'_>,
    ) -> zbus::Result<zvariant::OwnedObjectPath>;

    fn select_sources(
        &self,
        options: SelectSourcesOptions<'_>,
    ) -> zbus::Result<zvariant::OwnedObjectPath>;

    fn start(&self, options: StartOptions<'_>) -> zbus::Result<zvariant::OwnedObjectPath>;

    #[zbus(property)]
    fn available_cursor_modes(&self) -> zbus::Result<CursorModes>;

    #[zbus(property)]
    fn available_source_types(&self) -> zbus::Result<SourceType>;

    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;
}
