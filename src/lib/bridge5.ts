/// Phase 5 TypeScript bridge types and invoke wrappers

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ── Script types ──────────────────────────────────────────────────────────────

export type TriggerType =
    | "on_track_start"
    | "on_track_end"
    | "on_crossfade_start"
    | "on_queue_empty"
    | "on_request_received"
    | "on_hour"
    | "on_encoder_connect"
    | "on_encoder_disconnect"
    | "manual";

export interface Script {
    id: number;
    name: string;
    description?: string;
    content: string;
    enabled: boolean;
    trigger_type: TriggerType;
    last_run_at?: number;
    last_error?: string;
}

export interface ScriptRunResult {
    success: boolean;
    output: string[];
    error?: string;
    error_line?: number;
}

export interface ScriptLogEntry {
    level: "info" | "warn" | "error";
    message: string;
    timestamp: number;
}

// ── Mic / Voice types ─────────────────────────────────────────────────────────

export interface AudioDevice {
    name: string;
    is_default: boolean;
}

export interface MicConfig {
    device_name?: string;
    sample_rate: number;
    channels: number;
    gate_enabled: boolean;
    gate_threshold_db: number;
    gate_attack_ms: number;
    gate_release_ms: number;
    comp_enabled: boolean;
    comp_ratio: number;
    comp_threshold_db: number;
    comp_attack_ms: number;
    comp_release_ms: number;
    ptt_enabled: boolean;
    ptt_hotkey?: string;
}

export interface VoiceRecordingResult {
    filePath: string;
    durationMs: number;
}

// ── Script commands ───────────────────────────────────────────────────────────

export const getScripts = () => invoke<Script[]>("get_scripts");
export const saveScript = (script: Script) => invoke<number>("save_script", { script });
export const deleteScript = (id: number) => invoke<void>("delete_script", { id });
export const runScript = (id: number) => invoke<ScriptRunResult>("run_script", { id });
export const getScriptLog = (id: number, limit = 50) =>
    invoke<ScriptLogEntry[]>("get_script_log", { id, limit });

// ── Mic commands ──────────────────────────────────────────────────────────────

export const getAudioInputDevices = () => invoke<AudioDevice[]>("get_audio_input_devices");
export const getMicConfig = () => invoke<MicConfig>("get_mic_config");
export const setMicConfig = (config: MicConfig) => invoke<void>("set_mic_config", { config });
export const startMic = () => invoke<void>("start_mic");
export const stopMic = () => invoke<void>("stop_mic");
export const setPtt = (active: boolean) => invoke<void>("set_ptt", { active });

// ── Voice recording commands ──────────────────────────────────────────────────

export const startVoiceRecording = () => invoke<void>("start_voice_recording");
export const stopVoiceRecording = () => invoke<VoiceRecordingResult>("stop_voice_recording");
export const saveVoiceTrack = (filePath: string, title: string) =>
    invoke<number>("save_voice_track", { filePath, title });

// ── Events ────────────────────────────────────────────────────────────────────

export const onPttStateChanged = (handler: (e: { active: boolean }) => void) =>
    listen<{ active: boolean }>("ptt_state_changed", (ev) => handler(ev.payload));

export const onScriptLog = (handler: (e: { scriptId: number; level: string; message: string; timestamp: number }) => void) =>
    listen<{ scriptId: number; level: string; message: string; timestamp: number }>("script_log", (ev) => handler(ev.payload));

export const onMicLevel = (handler: (e: { leftDb: number; rightDb: number }) => void) =>
    listen<{ leftDb: number; rightDb: number }>("mic_level", (ev) => handler(ev.payload));
