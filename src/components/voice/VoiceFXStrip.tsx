/// VoiceFXStrip — always-visible mic strip in the main UI
/// Shows: device name, mic VU meter, PTT button, mute button, de-esser/reverb toggles

import { useState, useEffect, useRef } from "react";
import {
    MicConfig,
    getMicConfig,
    startMic,
    stopMic,
    setPtt,
    onMicLevel,
    onPttStateChanged,
} from "../../lib/bridge5";
import { register, unregister, isRegistered } from "@tauri-apps/plugin-global-shortcut";

interface Props {
    onOpenSettings: () => void;
    onOpenVtRecorder: () => void;
}

export function VoiceFXStrip({ onOpenSettings, onOpenVtRecorder }: Props) {
    const [config, setConfig] = useState<MicConfig | null>(null);
    const [active, setActive] = useState(false);  // mic stream running
    const [muted, setMuted] = useState(false);
    const [pptActive, setPptActive] = useState(false);
    const [levelL, setLevelL] = useState(0);
    const [levelR, setLevelR] = useState(0);
    const canvasRef = useRef<HTMLCanvasElement>(null);

    // Load config
    useEffect(() => {
        getMicConfig().then(setConfig).catch(() => { });
    }, []);

    // Subscribe to mic level events
    useEffect(() => {
        const unsub = onMicLevel((e) => {
            setLevelL(e.leftDb);
            setLevelR(e.rightDb);
        });
        return () => { unsub.then((fn) => fn()); };
    }, []);

    // Subscribe to PTT state changes
    useEffect(() => {
        const unsub = onPttStateChanged((e) => setPptActive(e.active));
        return () => { unsub.then((fn) => fn()); };
    }, []);

    // Global shortcut for PTT
    useEffect(() => {
        if (!config?.ptt_enabled || !config?.ptt_hotkey) return;
        const hotkey = config.ptt_hotkey;

        const setupHotkey = async () => {
            try {
                if (await isRegistered(hotkey)) await unregister(hotkey);

                await register(hotkey, async (e) => {
                    if (e.state === "Pressed") {
                        await setPtt(true);
                        setPptActive(true);
                    } else if (e.state === "Released") {
                        await setPtt(false);
                        setPptActive(false);
                    }
                });
            } catch (err) {
                console.error("Failed to register PTT hotkey:", err);
            }
        };

        setupHotkey();
        return () => {
            unregister(hotkey).catch(console.error);
        };
    }, [config?.ptt_enabled, config?.ptt_hotkey]);

    // Draw VU meter on canvas
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d")!;
        const W = canvas.width;
        const H = canvas.height;
        ctx.clearRect(0, 0, W, H);

        const drawBar = (x: number, w: number, level: number) => {
            // level is 0–1 (normalised from dB)
            const filled = Math.max(0, Math.min(1, level));
            const barH = filled * H;

            // Background
            ctx.fillStyle = "#181828";
            ctx.fillRect(x, 0, w, H);

            // Gradient bar
            const grad = ctx.createLinearGradient(0, H, 0, 0);
            grad.addColorStop(0, "#22c55e");
            grad.addColorStop(0.7, "#f59e0b");
            grad.addColorStop(1, "#ef4444");
            ctx.fillStyle = grad;
            ctx.fillRect(x, H - barH, w, barH);
        };

        // Normalise dB (-60..0) to 0..1
        const norm = (db: number) => Math.max(0, (db + 60) / 60);
        drawBar(0, W / 2 - 1, norm(levelL));
        drawBar(W / 2 + 1, W / 2 - 1, norm(levelR));
    }, [levelL, levelR]);

    const handleToggleMic = async () => {
        if (active) {
            await stopMic();
            setActive(false);
        } else {
            await startMic();
            setActive(true);
        }
    };

    const handlePtt = async (down: boolean) => {
        await setPtt(down);
        setPptActive(down);
    };

    return (
        <div style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "6px 12px",
            background: "var(--surface-1)",
            borderTop: "1px solid var(--border)",
            flexShrink: 0,
        }}>
            {/* Status dot */}
            <span style={{
                width: 8, height: 8, borderRadius: "50%",
                background: pptActive ? "var(--red)" : active ? "var(--green)" : "var(--text-muted)",
                flexShrink: 0,
                animation: pptActive ? "pulse-dot 1s infinite" : "none",
            }} />

            {/* Label */}
            <span style={{ fontSize: 10, color: "var(--text-secondary)", fontWeight: 600, minWidth: 28 }}>
                {pptActive ? "LIVE" : active ? "MIC" : "OFF"}
            </span>

            {/* VU meter */}
            <canvas
                ref={canvasRef}
                width={24}
                height={20}
                style={{
                    borderRadius: 2,
                    opacity: active ? 1 : 0.3,
                    transition: "opacity 0.3s",
                }}
            />

            {/* Device name */}
            <span style={{
                fontSize: 10,
                color: "var(--text-muted)",
                flex: 1,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
            }}>
                {config?.device_name ?? "Default Input"}
            </span>

            {/* PTT Button */}
            <button
                className="btn"
                style={{
                    padding: "3px 10px",
                    fontSize: 10,
                    fontWeight: 700,
                    background: pptActive ? "var(--red)" : "var(--surface-2)",
                    color: pptActive ? "#fff" : "var(--text-secondary)",
                    border: `1px solid ${pptActive ? "var(--red)" : "var(--border)"}`,
                    borderRadius: "var(--r-sm)",
                    cursor: "pointer",
                    userSelect: "none",
                    transition: "all 0.1s",
                }}
                onMouseDown={() => handlePtt(true)}
                onMouseUp={() => handlePtt(false)}
                onMouseLeave={() => { if (pptActive) handlePtt(false); }}
                title="Push To Talk"
            >
                🎙 PTT
            </button>

            {/* Mute */}
            <button
                className="btn btn-ghost"
                style={{
                    padding: "3px 8px", fontSize: 10,
                    color: muted ? "var(--red)" : "var(--text-muted)",
                }}
                onClick={() => setMuted((m) => !m)}
                title={muted ? "Unmute" : "Mute mic"}
            >
                {muted ? "🔇" : "🔈"}
            </button>

            {/* On/Off */}
            <button
                className={`btn ${active ? "btn-danger" : "btn-secondary"}`}
                style={{ padding: "3px 10px", fontSize: 10 }}
                onClick={handleToggleMic}
            >
                {active ? "■ Stop" : "▶ Start"}
            </button>

            {/* Record VT */}
            <button
                className="btn btn-ghost"
                style={{ padding: "3px 8px", fontSize: 11, color: "var(--red)" }}
                onClick={onOpenVtRecorder}
                title="Record Voice Track"
            >
                ● REC VT
            </button>

            {/* Settings */}
            <button
                className="btn btn-ghost"
                style={{ padding: "3px 8px", fontSize: 11 }}
                onClick={onOpenSettings}
                title="Mic settings"
            >
                ⚙
            </button>
        </div>
    );
}
