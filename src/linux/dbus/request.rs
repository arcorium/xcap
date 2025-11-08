use crate::platform::utils::Connection;
use crate::{XCapError, XCapResult};
use const_format::concatcp;
use pipewire::spa::spa_interface_call_method;
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use xcb::x::Error::Request;
use zbus::zvariant;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const REQUEST_PATH_PREFIX: &str = concatcp!(super::DESKTOP_PATH, "/request");

pub trait FromResponse: Sized {
    fn from_response(map: &mut HashMap<String, OwnedValue>) -> Self {
        Self::try_from_response(map).expect("Failed to deserialize response")
    }

    fn try_from_response(map: &mut HashMap<String, OwnedValue>) -> XCapResult<Self>;
}

#[derive(Debug, PartialEq, Eq, zvariant::Type, serde::Deserialize, Clone, Copy)]
#[repr(u32)]
pub enum ResponseCode {
    Success,
    Cancelled,
    Failed,
}

impl Display for ResponseCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseCode::Success => write!(f, "Success"),
            ResponseCode::Cancelled => write!(f, "Cancelled"),
            ResponseCode::Failed => write!(f, "Failed"),
        }
    }
}

impl ResponseCode {
    pub fn is_success(&self) -> bool {
        *self == ResponseCode::Success
    }
}

#[derive(serde::Deserialize, zvariant::Type, Debug)]
#[zvariant(signature = "ua{sv}")]
pub struct Responses<T> {
    pub code: ResponseCode,
    results: T, // It always Some, except after cast_resp
}

impl<T> Responses<T> {
    pub fn is_success(&self) -> bool {
        self.code.is_success()
    }
}

impl<T> Deref for Responses<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.results
    }
}

impl<T> DerefMut for Responses<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.results
    }
}

#[zbus::proxy(
    default_service = "org.freedesktop.portal.Desktop",
    interface = "org.freedesktop.portal.Request"
)]
pub trait Request {
    fn close(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn response(
        &self,
        code: ResponseCode,
        results: HashMap<String, zvariant::OwnedValue>,
    ) -> zbus::Result<()>;
}

impl ResponseArgs<'_> {
    pub fn is_success(&self) -> bool {
        self.code.is_success()
    }

    pub fn deserialize<T: FromResponse>(&mut self) -> T {
        T::from_response(self)
    }

    pub fn try_deserialize<T: FromResponse>(&mut self) -> XCapResult<T> {
        T::try_from_response(self)
    }
}

impl Deref for ResponseArgs<'_> {
    type Target = HashMap<String, zvariant::OwnedValue>;

    fn deref(&self) -> &Self::Target {
        &self.results
    }
}

impl DerefMut for ResponseArgs<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.results
    }
}

pub fn request_handle_path(conn: &Connection, token: &str) -> XCapResult<OwnedObjectPath> {
    let fmt = format!("{}/{}/{}", REQUEST_PATH_PREFIX, conn.unique_name(), token);
    Ok(OwnedObjectPath::try_from(fmt)?)
}

pub fn on_blocking_response<T: FromResponse, F>(
        conn: &Connection,
        token: &str,
        f: F,
    ) -> XCapResult<Responses<T>>
    where
        F: FnOnce() -> XCapResult<()>,
    {
        let request_handle_path = request_handle_path(conn, token)?;
        let proxy = RequestProxyBlocking::new(conn, request_handle_path)?;
        let mut resp_it = proxy.receive_response()?;

        f()?;

        let resp = resp_it
            .next()
            .ok_or_else(|| XCapError::new("No response received"))?;

        let mut resp = resp.args()?;

        if !resp.is_success() {
            return Err(XCapError::new("Response code is not success"));
        }

        Ok(Responses::<T> {
            code: resp.code,
            results: resp.try_deserialize()?,
        })
    }

/*
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use zbus::zvariant::Value;

    #[test]
    fn test_create_session() -> anyhow::Result<()> {
        let conn = zbus::blocking::Connection::session()?;
        let screencast_proxy = PortalScreenCastProxyBlocking::new(&conn)?;

        let handle_token = format!("xcap_{}", rand::random::<u32>());
        let session_handle_token = format!("xcap_{}", rand::random::<u32>());
        let id = conn
            .unique_name()
            .ok_or_else(|| anyhow!("No unique name"))?
            .trim_start_matches(":")
            .replace(".", "_");

        let req_handle_path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            id, handle_token
        );

        let request_proxy = PortalRequestProxyBlocking::new(&conn, req_handle_path)?;

        let mut resp_it = request_proxy.receive_response()?;

        let param = CreateSessionOption {
            handle_token: &handle_token,
            session_handle_token: &session_handle_token,
        };
        let handle_path = screencast_proxy.create_session(param)?;

        println!("Unique ID: {id}");

        println!("Handle path {:?}", handle_path);

        /*let ResponseArgs {
            phantom,
            response,
            results,
        } = resp_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("No response received"))?
            .args()?;*/

        let results = resp_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("No response received"))?
            .args()?
            .resp;

        let response = results.code;
        if response != 0 {
            return Err(anyhow::anyhow!("Response code is {:?}", response));
        }

        println!("Results {:?}", results);
        // request_proxy.close()?;
        Ok(())
    }
}
*/
