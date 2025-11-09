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
        on_blocking_response, ResponseCode, Responses,
    };
    use crate::platform::dbus::screencast::{CreateSessionOption, CreateSessionResponse, CursorModes, PersistMode, ScreenCastProxyBlocking, SelectSourcesOption, SourceType, StartOption, StartResponse};
    use crate::platform::dbus::session::{session_handle_path, SessionProxyBlocking};
    use crate::platform::dbus::{generate_session_handle, generate_token_handle};
    use crate::platform::utils::{get_zbus_connection, Connection};
    use zbus::zvariant::{ObjectPath, OwnedObjectPath};

    fn create_session<'a>(
        conn: &Connection,
    ) -> anyhow::Result<(ScreenCastProxyBlocking<'a>, OwnedObjectPath)> {
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

        Ok((proxy, session_handle_path))
    }

    #[test]
    fn create_session_works() -> anyhow::Result<()> {
        let conn = get_zbus_connection();
        let (proxy, session_path) = create_session(conn)?;

        let sess_proxy = SessionProxyBlocking::new(conn, session_path)?;
        sess_proxy.close()?;

        Ok(())
    }

    fn select_sources<'a>(
        conn: &Connection,
        session: ObjectPath<'a>,
        proxy: &ScreenCastProxyBlocking<'a>,
        mut opt: SelectSourcesOption,
    ) -> anyhow::Result<()> {
        let handle_token = generate_token_handle();
        let resp: Responses<()> = on_blocking_response(conn, handle_token.as_str(), || {
            opt.handle_token = &handle_token;
            proxy.select_sources(session.as_ref(), opt)?;
            Ok(())
        })?;

        assert_eq!(resp.code, ResponseCode::Success);
        Ok(())
    }

    #[test]
    fn select_sources_works() -> anyhow::Result<()> {
        let conn = get_zbus_connection();
        let (proxy, session_path) = create_session(conn)?;
        select_sources(conn, session_path.as_ref(), &proxy, SelectSourcesOption {
            handle_token: "", // handled by the callee
            types: SourceType::Monitor,
            multiple: false,
            cursor_mode: CursorModes::Hidden,
            restore_token: None,
            persist_mode: PersistMode::None,
        })?;

        let sess_proxy = SessionProxyBlocking::new(conn, session_path)?;
        sess_proxy.close()?;

        Ok(())
    }

    #[test]
    fn cancelled_start_screen_cast_single_monitor() -> anyhow::Result<()> {
        let conn = get_zbus_connection();
        let (proxy, session_path) = create_session(conn)?;
        select_sources(conn, session_path.as_ref(), &proxy, SelectSourcesOption {
            handle_token: "", // handled by the callee
            types: SourceType::Monitor,
            multiple: false,
            cursor_mode: CursorModes::Hidden,
            restore_token: None,
            persist_mode: PersistMode::None,
        })?;

        let handle_token = generate_token_handle();
        println!("start handle token: {}", handle_token);
        let resp: Responses<StartResponse> = on_blocking_response(conn, handle_token.as_str(), || {
            proxy.start(session_path.as_ref(), "", StartOption {
                handle_token: &handle_token,
            })?;

            Ok(())
        })?;

        assert_eq!(resp.code, ResponseCode::Cancelled);

        Ok(())
    }

    #[test]
    fn start_screen_cast_single_monitor() -> anyhow::Result<()> {
        let conn = get_zbus_connection();
        let (proxy, session_path) = create_session(conn)?;
        select_sources(conn, session_path.as_ref(), &proxy, SelectSourcesOption {
            handle_token: "", // handled by the callee
            types: SourceType::Monitor | SourceType::Window,
            multiple: true,
            cursor_mode: CursorModes::Hidden,
            restore_token: None,
            persist_mode: PersistMode::None,
        })?;

        let handle_token = generate_token_handle();
        let resp: Responses<StartResponse> = on_blocking_response(conn, handle_token.as_str(), || {
            proxy.start(session_path.as_ref(), "", StartOption {
                handle_token: &handle_token,
            })?;

            Ok(())
        })?;

        assert_eq!(resp.code, ResponseCode::Success);
        assert!(resp.streams.is_some());
        assert!(resp.restore_token.is_none());

        Ok(())
    }

    #[test]
    fn start_screen_cast_single_monitor_persist() -> anyhow::Result<()> {
        let conn = get_zbus_connection();
        let (mut proxy, mut session_path) = create_session(conn)?;
        select_sources(conn, session_path.as_ref(), &proxy, SelectSourcesOption {
            handle_token: "", // handled by the callee
            types: SourceType::Monitor,
            multiple: false,
            cursor_mode: CursorModes::Hidden,
            restore_token: None,
            persist_mode: PersistMode::Session,
        })?;

        let handle_token = generate_token_handle();
        let resp: Responses<StartResponse> = on_blocking_response(conn, handle_token.as_str(), || {
            proxy.start(session_path.as_ref(), "", StartOption {
                handle_token: &handle_token,
            })?;

            Ok(())
        })?;

        assert_eq!(resp.code, ResponseCode::Success);
        assert!(resp.streams.is_some());
        assert!(resp.restore_token.is_some());

        std::thread::sleep(std::time::Duration::from_secs(1));

        SessionProxyBlocking::new(conn, session_path.as_ref())?.
            close()?;

        std::thread::sleep(std::time::Duration::from_secs(1));

        (proxy, session_path) = create_session(conn)?;
        select_sources(conn, session_path.as_ref(), &proxy, SelectSourcesOption {
            handle_token: "", // handled by the callee
            types: SourceType::Monitor,
            multiple: false,
            cursor_mode: CursorModes::Hidden,
            restore_token: Some(resp.restore_token.as_ref().unwrap().as_ref()),
            persist_mode: PersistMode::Session,
        })?;

        // It should not display dialog
        let resp: Responses<StartResponse> = on_blocking_response(conn, handle_token.as_str(), || {
            proxy.start(session_path.as_ref(), "", StartOption {
                handle_token: &handle_token,
            })?;

            Ok(())
        })?;

        assert_eq!(resp.code, ResponseCode::Success);
        assert!(resp.streams.is_some());
        assert!(resp.restore_token.is_some());

        Ok(())
    }
}
