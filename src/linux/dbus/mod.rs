use const_format::concatcp;

pub mod request;
pub mod screencast;
pub mod session;

const FREEDESKTOP_PATH: &str = "/org/freedesktop";
const PORTAL_PATH: &str = concatcp!(FREEDESKTOP_PATH, "/portal");
const DESKTOP_PATH: &str = concatcp!(PORTAL_PATH, "/desktop");

const TOKEN_HANDLE_PREFIX: &str = "xcap_";
const SESSION_TOKEN_HANDLE_PREFIX: &str = concatcp!(TOKEN_HANDLE_PREFIX, "sess_");

pub fn generate_token_handle() -> String {
    format!("{}{}", TOKEN_HANDLE_PREFIX, rand::random::<u32>())
}

pub fn generate_session_handle() -> String {
    format!("{}{}", SESSION_TOKEN_HANDLE_PREFIX, rand::random::<u32>())
}

#[cfg(test)]
mod tests {
    use crate::platform::dbus::request::{
        ResponseCode, Responses, on_blocking_response, request_handle_path,
    };
    use crate::platform::dbus::screencast::{
        CreateSessionOption, CreateSessionResponse, ScreenCastProxyBlocking,
    };
    use crate::platform::dbus::session::{SessionProxyBlocking, session_handle_path};
    use crate::platform::dbus::{generate_session_handle, generate_token_handle, request};
    use crate::platform::utils::{Connection, get_zbus_connection};
    use crate::{XCapError, XCapResult};
    use std::collections::HashMap;
    use zbus::zvariant;

    #[test]
    fn create_session_works() -> anyhow::Result<()> {
        let conn = get_zbus_connection();
        let proxy = ScreenCastProxyBlocking::new(conn)?;
        let handle_token = generate_token_handle();
        let session_token = generate_session_handle();
        let session_handle_path = session_handle_path(conn, &session_token)?;

        let resp: Responses<CreateSessionResponse> =
            on_blocking_response(conn, handle_token.as_str(), || {
                proxy.create_session(CreateSessionOption {
                    handle_token: &handle_token,
                    session_handle_token: &session_token,
                })?;

                Ok(())
            })?;

        assert_eq!(resp.code, ResponseCode::Success);
        assert_eq!(resp.session_handle, session_handle_path.as_str());

        let sess_proxy = SessionProxyBlocking::new(conn, session_handle_path)?;
        sess_proxy.close()?;

        Ok(())
    }
}
