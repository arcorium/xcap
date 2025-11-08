use crate::platform::utils::Connection;
use const_format::concatcp;
use std::collections::HashMap;
use zbus::zvariant;
use zbus::zvariant::OwnedObjectPath;

const SESSION_PATH_PREFIX: &str = concatcp!(super::DESKTOP_PATH, "/session");

#[zbus::proxy(
    default_service = "org.freedesktop.portal.Desktop",
    interface = "org.freedesktop.portal.Session"
)]
pub trait Session {
    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;

    fn close(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn details(&self, details: HashMap<&str, zvariant::Value<'_>>) -> zbus::Result<()>;
}

pub fn session_handle_path(conn: &Connection, token: &str) -> zbus::Result<OwnedObjectPath> {
    let fmt = format!("{}/{}/{}", SESSION_PATH_PREFIX, conn.unique_name(), token);
    Ok(OwnedObjectPath::try_from(fmt)?)
}
