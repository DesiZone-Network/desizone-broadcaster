/// VoiceTrackRecorder — record a voice back-announce / voice track
/// Records from the selected mic, shows waveform feedback, then saves to library.

import { useState, useEffect, useRef } from "react";
import {
    startVoiceRecording,
    stopVoiceRecording,
    saveVoiceTrack,
    VoiceRecordingResult,
} from "../../lib/bridge5";

interface Props {
    onClose: () => void;
    onSaved?: (filePath: string, title: string) => void;
}

function formatMs(ms: number): string {
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const ss = s % 60;
    return `${String(m).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
}

export function VoiceTrackRecorder({ onClose, onSaved }: Props) {
    type State = "idle" | "recording" | "done";
    const [state, setState] = useState<State>("idle");
    const [elapsed, setElapsed] = useState(0);
    const [result, setResult] = useState<VoiceRecordingResult | null>(null);
    const [title, setTitle] = useState("");
    const [saving, setSaving] = useState(false);
    const [saveError, setSaveError] = useState<string | null>(null);

    const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

    // Cleanup on unmount
    useEffect(() => () => {
        if (timerRef.current) clearInterval(timerRef.current);
    }, []);

    const handleStart = async () => {
        await startVoiceRecording();
        setState("recording");
        setElapsed(0);
        timerRef.current = setInterval(() => setElapsed((e) => e + 100), 100);
    };

    const handleStop = async () => {
        if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null; }
        const res = await stopVoiceRecording();
        setResult(res);
        setState("done");
    };

    const handleSave = async () => {
        if (!result || !title.trim()) return;
        setSaving(true);
        setSaveError(null);
        try {
            await saveVoiceTrack(result.filePath, title.trim());
            onSaved?.(result.filePath, title.trim());
            onClose();
        } catch (e) {
            setSaveError(String(e));
        } finally {
            setSaving(false);
        }
    };

    const handleDiscard = () => {
        setState("idle");
        setResult(null);
        setElapsed(0);
        setTitle("");
        setSaveError(null);
    };

    return (
        <div style={{
            position: "fixed", inset: 0, zIndex: 1100,
            background: "rgba(0,0,0,0.75)",
            display: "flex", alignItems: "center", justifyContent: "center",
        }}>
            <div style={{
                width: "min(480px, 94vw)",
                background: "var(--surface-1)",
                border: "1px solid var(--border)",
                borderRadius: "var(--r-lg)",
                overflow: "hidden",
                boxShadow: "0 24px 64px rgba(0,0,0,0.6)",
            }}>
                {/* Header */}
                <div style={{
                    padding: "14px 20px",
                    borderBottom: "1px solid var(--border)",
                    display: "flex", alignItems: "center",
                }}>
                    <span style={{ flex: 1, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
                        🎙 Voice Track Recorder
                    </span>
                    <button className="btn btn-ghost" style={{ padding: "3px 10px", fontSize: 11 }} onClick={onClose}>✕</button>
                </div>

                <div style={{ padding: 24 }}>
                    {/* Timer display */}
                    <div style={{
                        textAlign: "center",
                        marginBottom: 24,
                    }}>
                        <div style={{
                            fontSize: 48,
                            fontFamily: "var(--font-mono)",
                            fontWeight: 700,
                            color: state === "recording" ? "var(--red)" : "var(--text-primary)",
                            letterSpacing: 4,
                        }}>
                            {state === "done" && result
                                ? formatMs(result.durationMs)
                                : formatMs(elapsed)}
                        </div>
                        {state === "recording" && (
                            <div style={{
                                display: "flex", alignItems: "center", justifyContent: "center",
                                gap: 6, marginTop: 6,
                            }}>
                                <span style={{
                                    width: 8, height: 8, borderRadius: "50%",
                                    background: "var(--red)",
                                    animation: "pulse-dot 1s infinite",
                                }} />
                                <span style={{ fontSize: 11, color: "var(--red)", fontWeight: 600 }}>RECORDING</span>
                            </div>
                        )}
                        {state === "done" && (
                            <div style={{ fontSize: 11, color: "var(--green)", marginTop: 6 }}>✓ Recording complete</div>
                        )}
                    </div>

                    {/* Controls */}
                    {state === "idle" && (
                        <div style={{ display: "flex", justifyContent: "center" }}>
                            <button
                                className="btn btn-danger"
                                style={{ padding: "10px 32px", fontSize: 13, borderRadius: "var(--r-lg)" }}
                                onClick={handleStart}
                            >
                                ● Start Recording
                            </button>
                        </div>
                    )}

                    {state === "recording" && (
                        <div style={{ display: "flex", justifyContent: "center" }}>
                            <button
                                className="btn btn-secondary"
                                style={{ padding: "10px 32px", fontSize: 13, borderRadius: "var(--r-lg)" }}
                                onClick={handleStop}
                            >
                                ■ Stop
                            </button>
                        </div>
                    )}

                    {state === "done" && (
                        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                            <div>
                                <label style={{ fontSize: 10, color: "var(--text-muted)", display: "block", marginBottom: 4 }}>TITLE FOR LIBRARY</label>
                                <input
                                    className="input"
                                    value={title}
                                    onChange={(e) => setTitle(e.target.value)}
                                    placeholder="e.g. Top-of-hour back announce"
                                    style={{ width: "100%", fontSize: 12 }}
                                    autoFocus
                                />
                            </div>
                            {saveError && (
                                <div style={{ fontSize: 11, color: "var(--red)" }}>✕ {saveError}</div>
                            )}
                            <div style={{ display: "flex", gap: 8 }}>
                                <button className="btn btn-ghost" style={{ flex: 1, padding: "7px", fontSize: 12 }} onClick={handleDiscard}>
                                    Discard
                                </button>
                                <button
                                    className="btn btn-primary"
                                    style={{ flex: 1, padding: "7px", fontSize: 12 }}
                                    onClick={handleSave}
                                    disabled={saving || !title.trim()}
                                >
                                    {saving ? "Saving…" : "Save to Library"}
                                </button>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
