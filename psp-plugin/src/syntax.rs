use psp_lua_sys::ffi::*;
use serde::Serialize;

use crate::sandbox::{read_string, Cancel, Limits, Sandbox};

/// The leading `=` makes Lua use the rest verbatim in error positions, so a
/// failure message begins with [`PREFIX`].
const CHUNK_NAME: &std::ffi::CStr = c"=psp";
const PREFIX: &str = "psp:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyntaxError {
    pub line: Option<u32>,
    pub message: String,
}

pub fn check(source: &str) -> Option<SyntaxError> {
    let Some(sandbox) = Sandbox::new(Limits::default(), Cancel::new()) else {
        return Some(SyntaxError {
            line: None,
            message: "the Lua parser could not be created".to_string(),
        });
    };

    let raw = unsafe {
        let loaded = luaL_loadbufferx(
            sandbox.as_ptr(),
            source.as_ptr().cast(),
            source.len(),
            CHUNK_NAME.as_ptr(),
            c"t".as_ptr(),
        );
        if loaded == LUA_OK {
            return None;
        }
        read_string(sandbox.as_ptr(), -1)
    };

    Some(split_position(
        raw.as_deref().unwrap_or("the Lua parser reported an error with no message"),
    ))
}

fn split_position(raw: &str) -> SyntaxError {
    let unpositioned = || SyntaxError { line: None, message: raw.to_string() };

    let Some(rest) = raw.strip_prefix(PREFIX) else {
        return unpositioned();
    };
    let Some((digits, message)) = rest.split_once(": ") else {
        return unpositioned();
    };
    match digits.parse::<u32>() {
        Ok(line) => SyntaxError { line: Some(line), message: message.to_string() },
        Err(_) => unpositioned(),
    }
}

#[cfg(test)]
mod tests {
    use super::split_position;

    #[test]
    fn a_positioned_message_splits_into_line_and_text() {
        let split = split_position("psp:12: unexpected symbol near '='");
        assert_eq!(split.line, Some(12));
        assert_eq!(split.message, "unexpected symbol near '='");
    }

    #[test]
    fn a_message_without_the_prefix_is_kept_whole() {
        let split = split_position("not enough memory");
        assert_eq!(split.line, None);
        assert_eq!(split.message, "not enough memory");
    }

    #[test]
    fn a_non_numeric_position_is_kept_whole() {
        let split = split_position("psp:abc: something");
        assert_eq!(split.line, None);
        assert_eq!(split.message, "psp:abc: something");
    }
}
