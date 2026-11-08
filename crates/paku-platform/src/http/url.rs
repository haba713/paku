// SPDX-License-Identifier: GPL-2.0-or-later

//! Splitting `http(s)://host[:port]/path` URLs for WinHTTP.

/// Parts of an HTTP URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    /// `true` for `https`.
    pub secure: bool,
    /// Host name or IP literal (IPv6 without brackets).
    pub host: String,
    /// Port, defaulting to 80/443.
    pub port: u16,
    /// Path plus query, always starting with `/`.
    pub path: String,
}

/// Parses an `http://` or `https://` URL. User info and fragments are rejected
/// or dropped; they never appear in package repository URLs.
pub fn parse(url: &str) -> Result<Url, String> {
    let (secure, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("unsupported URL scheme: {url}"));
    };
    let rest = rest.split('#').next().unwrap_or_default();
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || authority.contains('@') {
        return Err(format!("invalid URL host: {url}"));
    }
    let (host, port) = split_port(authority, secure).ok_or_else(|| format!("invalid URL port: {url}"))?;
    Ok(Url {
        secure,
        host: host.to_string(),
        port,
        path: path.to_string(),
    })
}

fn split_port(authority: &str, secure: bool) -> Option<(&str, u16)> {
    let default = if secure { 443 } else { 80 };
    if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None if after.is_empty() => default,
            None => return None,
        };
        return Some((host, port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => Some((host, port.parse().ok()?)),
        None => Some((authority, default)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls() {
        let u = parse("https://mirror.msys2.org/mingw/ucrt64/ucrt64.db").unwrap();
        assert_eq!(
            u,
            Url {
                secure: true,
                host: "mirror.msys2.org".into(),
                port: 443,
                path: "/mingw/ucrt64/ucrt64.db".into()
            }
        );
        let u = parse("http://localhost:8080").unwrap();
        assert_eq!((u.secure, u.port, u.path.as_str()), (false, 8080, "/"));
        let u = parse("http://[::1]:81/x?y#z").unwrap();
        assert_eq!((u.host.as_str(), u.port, u.path.as_str()), ("::1", 81, "/x?y"));
    }

    #[test]
    fn rejects_bad_urls() {
        for url in ["ftp://x/y", "https://", "https://u:p@host/", "https://h:99999/"] {
            assert!(parse(url).is_err(), "{url}");
        }
    }
}
