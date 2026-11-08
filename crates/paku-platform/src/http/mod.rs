// SPDX-License-Identifier: GPL-2.0-or-later

//! Blocking HTTP(S) GET over Windows' WinHTTP.
//!
//! WinHTTP uses the system TLS stack, certificate store and proxy settings,
//! and follows redirects, so paku needs no TLS crate and no C toolchain.

pub mod url;

#[cfg(windows)]
mod ffi;
#[cfg(windows)]
mod win;

use std::io;
use std::time::Duration;

/// HTTP client session.
pub struct Client {
    #[cfg(windows)]
    inner: win::Session,
}

/// Response headers of interest plus the streaming body ([`io::Read`]).
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Raw `Last-Modified` header.
    pub last_modified: Option<String>,
    /// `Content-Length`, if sent.
    pub content_length: Option<u64>,
    #[cfg(windows)]
    body: win::Body,
}

impl Client {
    /// Opens a session. `timeout` bounds name resolution, connecting, sending
    /// and each wait for received data; `None` waits forever.
    pub fn new(user_agent: &str, timeout: Option<Duration>) -> io::Result<Self> {
        #[cfg(windows)]
        return Ok(Client {
            inner: win::Session::open(user_agent, timeout)?,
        });
        #[cfg(not(windows))]
        {
            let _ = (user_agent, timeout);
            Err(io::Error::new(io::ErrorKind::Unsupported, "HTTP requires Windows"))
        }
    }

    /// Sends a GET request with extra `headers` and returns once the response
    /// headers have arrived. Non-2xx statuses are returned, not errors.
    pub fn get(&self, url: &str, headers: &[(&str, &str)]) -> io::Result<Response> {
        let parsed = url::parse(url).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        #[cfg(windows)]
        return self.inner.get(&parsed, headers);
        #[cfg(not(windows))]
        {
            let _ = (parsed, headers);
            Err(io::Error::new(io::ErrorKind::Unsupported, "HTTP requires Windows"))
        }
    }
}

impl io::Read for Response {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(windows)]
        return self.body.read(buf);
        #[cfg(not(windows))]
        {
            let _ = buf;
            Ok(0)
        }
    }
}
