/// `scripting/` — Phase 5 Lua scripting engine
///
/// Each script runs in its own isolated Lua VM (mlua).
/// Scripts are triggered by Rust events dispatched from `ScriptEngine`.
/// Script errors are caught and logged — never crash the audio engine.
pub mod api;
pub mod engine;
pub mod sandbox;
pub mod trigger;
