// SPDX-License-Identifier: GPL-2.0-or-later

//! Terminal detection that also recognizes Cygwin/MSYS ptys.
//!
//! Git Bash runs in mintty, where native Windows programs see named pipes
//! (`\msys-<hash>-pty0-to-master`) instead of a console, so the standard
//! `IsTerminal` check reports `false`. This is the same check Git for Windows
//! uses to decide whether to page and colorize.

use std::io::IsTerminal;

/// A standard stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Standard input.
    Stdin,
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Whether `stream` is an interactive terminal (console or mintty pty).
pub fn is_terminal(stream: Stream) -> bool {
    let console = match stream {
        Stream::Stdin => std::io::stdin().is_terminal(),
        Stream::Stdout => std::io::stdout().is_terminal(),
        Stream::Stderr => std::io::stderr().is_terminal(),
    };
    console || is_msys_pty(stream)
}

/// Whether a pipe name belongs to a Cygwin/MSYS pty.
pub fn is_pty_pipe_name(name: &str) -> bool {
    let name = name.trim_start_matches('\\');
    let runtime = name.starts_with("msys-") || name.starts_with("cygwin-");
    let pty = name.contains("-pty") && (name.ends_with("-to-master") || name.ends_with("-from-master"));
    runtime && pty
}

#[cfg(windows)]
fn is_msys_pty(stream: Stream) -> bool {
    use crate::ffi::*;

    let which = match stream {
        Stream::Stdin => STD_INPUT_HANDLE,
        Stream::Stdout => STD_OUTPUT_HANDLE,
        Stream::Stderr => STD_ERROR_HANDLE,
    };
    // FILE_NAME_INFO: u32 byte length followed by UTF-16 name
    let mut buf = [0u32; 260];
    // SAFETY: the handle comes from GetStdHandle and is only queried; `buf` is
    // large enough and properly aligned for FILE_NAME_INFO.
    let named = unsafe {
        let handle = GetStdHandle(which);
        if handle.is_null() || GetFileType(handle) != FILE_TYPE_PIPE {
            return false;
        }
        let size = (buf.len() * 4) as u32;
        GetFileInformationByHandleEx(handle, FILE_NAME_INFO_CLASS, buf.as_mut_ptr().cast(), size) != 0
    };
    if !named {
        return false;
    }
    let len_units = (buf[0] as usize / 2).min((buf.len() - 1) * 2);
    // SAFETY: the name starts right after the length field and holds len_units u16s.
    let units = unsafe { std::slice::from_raw_parts(buf.as_ptr().add(1).cast::<u16>(), len_units) };
    is_pty_pipe_name(&String::from_utf16_lossy(units))
}

#[cfg(not(windows))]
fn is_msys_pty(_stream: Stream) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_pipe_names() {
        assert!(is_pty_pipe_name("\\msys-dd50a72ab4668b33-pty0-to-master"));
        assert!(is_pty_pipe_name("\\cygwin-e022582115c10879-pty4-from-master"));
        assert!(!is_pty_pipe_name("\\msys-dd50a72ab4668b33-pipe-0x1"));
        assert!(!is_pty_pipe_name("\\other-pty0-to-master"));
    }
}
