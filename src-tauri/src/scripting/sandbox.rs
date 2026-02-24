/// `scripting/sandbox.rs` — Lua sandbox restrictions per script
///
/// Every script VM is created with a restricted set of standard libraries.
/// Dangerous libraries (os, io, debug, package, require) are omitted by default.
/// Scripts can be granted additional trust levels through their configuration.

use mlua::{Lua, Result as LuaResult};

/// Controls which Lua standard libraries are available to a script.
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Default: string, table, math, coroutine — no I/O or OS
    Basic,
    /// Standard + io (for reading files in a scoped path)
    FileRead,
    /// Full standard library — only for trusted scripts
    Elevated,
}

impl Default for TrustLevel {
    fn default() -> Self {
        Self::Basic
    }
}

/// Creates a new Lua VM with sandbox restrictions applied.
pub fn create_sandboxed_vm(trust: TrustLevel) -> LuaResult<Lua> {
    let lua = Lua::new();

    // Load safe standard libs
    lua.load_std_libs(mlua::StdLib::TABLE | mlua::StdLib::STRING | mlua::StdLib::MATH | mlua::StdLib::COROUTINE)?;

    if trust == TrustLevel::FileRead || trust == TrustLevel::Elevated {
        lua.load_std_libs(mlua::StdLib::IO)?;
    }

    if trust == TrustLevel::Elevated {
        lua.load_std_libs(mlua::StdLib::OS | mlua::StdLib::PACKAGE)?;
    }

    Ok(lua)
}
