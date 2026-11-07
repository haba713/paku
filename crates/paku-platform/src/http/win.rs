// SPDX-License-Identifier: MIT

//! WinHTTP-backed implementation of [`super::Client`].

use std::io;
use std::ptr::{null, null_mut};
use std::time::Duration;

use super::Response;
use super::ffi::*;
use super::url::Url;

/// Owned WinHTTP handle, closed on drop.
struct Handle(HInternet);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by WinHTTP and is closed exactly once.
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

impl Handle {
    fn new(raw: HInternet, what: &str) -> io::Result<Self> {
        if raw.is_null() {
            return Err(last_error(what));
        }
        Ok(Handle(raw))
    }
}

pub struct Session(Handle);

/// Response body; field order makes the request close before its connection.
pub struct Body {
    request: Handle,
    _connect: Handle,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn check(ok: i32, what: &str) -> io::Result<()> {
    if ok == 0 { Err(last_error(what)) } else { Ok(()) }
}

fn last_error(what: &str) -> io::Error {
    // SAFETY: GetLastError has no preconditions.
    let code = unsafe { GetLastError() };
    let (kind, name) = match code {
        12002 => (io::ErrorKind::TimedOut, "operation timed out"),
        12007 => (io::ErrorKind::NotFound, "host name could not be resolved"),
        12029 => (io::ErrorKind::ConnectionRefused, "cannot connect"),
        12030 => (io::ErrorKind::ConnectionAborted, "connection aborted"),
        12175 => (io::ErrorKind::Other, "TLS/certificate error"),
        _ => (io::ErrorKind::Other, "WinHTTP error"),
    };
    io::Error::new(kind, format!("{what}: {name} ({code})"))
}

impl Session {
    pub fn open(user_agent: &str, timeout: Option<Duration>) -> io::Result<Self> {
        let agent = wide(user_agent);
        // SAFETY: `agent` is a NUL-terminated UTF-16 string that outlives the call.
        let raw = unsafe { WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0) };
        let session = Handle::new(raw, "WinHttpOpen")?;
        let ms = timeout.map_or(0, |t| i32::try_from(t.as_millis()).unwrap_or(i32::MAX));
        // SAFETY: valid session handle.
        check(
            unsafe { WinHttpSetTimeouts(session.0, ms, ms, ms, ms) },
            "WinHttpSetTimeouts",
        )?;
        Ok(Session(session))
    }

    pub fn get(&self, url: &Url, headers: &[(&str, &str)]) -> io::Result<Response> {
        let host = wide(&url.host);
        // SAFETY: valid session handle and NUL-terminated host string.
        let connect = Handle::new(
            unsafe { WinHttpConnect(self.0.0, host.as_ptr(), url.port, 0) },
            "connect",
        )?;
        let (verb, path) = (wide("GET"), wide(&url.path));
        let flags = if url.secure { WINHTTP_FLAG_SECURE } else { 0 };
        // SAFETY: valid connection handle; strings are NUL-terminated and outlive the call.
        let raw = unsafe { WinHttpOpenRequest(connect.0, verb.as_ptr(), path.as_ptr(), null(), null(), null(), flags) };
        let request = Handle::new(raw, "open request")?;
        send(&request, headers)?;
        let body = Body {
            request,
            _connect: connect,
        };
        Ok(Response {
            status: query_number(&body.request, WINHTTP_QUERY_STATUS_CODE)?.unwrap_or(0) as u16,
            last_modified: query_string(&body.request, WINHTTP_QUERY_LAST_MODIFIED)?,
            content_length: query_string(&body.request, WINHTTP_QUERY_CONTENT_LENGTH)?.and_then(|v| v.parse().ok()),
            body,
        })
    }
}

fn send(request: &Handle, headers: &[(&str, &str)]) -> io::Result<()> {
    let text: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
    let text = wide(&text);
    let (ptr, len) = if headers.is_empty() {
        (null(), 0)
    } else {
        (text.as_ptr(), HEADERS_LENGTH_NUL_TERMINATED)
    };
    // SAFETY: valid request handle; `text` outlives the call; no request body.
    check(
        unsafe { WinHttpSendRequest(request.0, ptr, len, null(), 0, 0, 0) },
        "send request",
    )?;
    // SAFETY: valid request handle after a successful send.
    check(
        unsafe { WinHttpReceiveResponse(request.0, null_mut()) },
        "receive response",
    )
}

fn query_number(request: &Handle, info: u32) -> io::Result<Option<u32>> {
    let mut value = 0u32;
    let mut len = 4u32;
    // SAFETY: `value` is a valid 4-byte buffer and `len` holds its size.
    let ok = unsafe {
        WinHttpQueryHeaders(
            request.0,
            info | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&raw mut value).cast(),
            &mut len,
            null_mut(),
        )
    };
    header_result(ok, value, "query headers")
}

fn query_string(request: &Handle, info: u32) -> io::Result<Option<String>> {
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: `buf` is valid for `len` bytes.
    let ok = unsafe { WinHttpQueryHeaders(request.0, info, null(), buf.as_mut_ptr().cast(), &mut len, null_mut()) };
    let chars = (len as usize / 2).min(buf.len());
    header_result(ok, String::from_utf16_lossy(&buf[..chars]), "query headers")
}

/// A missing header is `None`, other failures are errors.
fn header_result<T>(ok: i32, value: T, what: &str) -> io::Result<Option<T>> {
    if ok != 0 {
        return Ok(Some(value));
    }
    // SAFETY: GetLastError has no preconditions.
    if unsafe { GetLastError() } == ERROR_WINHTTP_HEADER_NOT_FOUND {
        return Ok(None);
    }
    Err(last_error(what))
}

impl Body {
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let want = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut got = 0u32;
        // SAFETY: `buf` is valid for `want` bytes; valid request handle.
        check(
            unsafe { WinHttpReadData(self.request.0, buf.as_mut_ptr().cast(), want, &mut got) },
            "read data",
        )?;
        Ok(got as usize)
    }
}
