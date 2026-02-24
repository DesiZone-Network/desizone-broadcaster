import { useState, useEffect, useRef, useCallback } from "react";
import {
    Play, Pause, Square, SkipBack, SkipForward,
    Volume2, Headphones, Radio, Music2
} from "lucide-react";
import {
    playDeck, pauseDeck, seekDeck, setChannelGain,
    onDeckStateChanged, onVuMeter,
    loadTrack,
    DeckId, DeckStateEvent, VuEvent,
} from "../../lib/bridge";
import { WaveformCanvas } from "./WaveformCanvas";
import { VUMeter } from "./VUMeter";

interface Props {
    deckId: DeckId;
    label: string;
    accentColor?: string;
    isOnAir?: boolean;
}

function formatTime(ms: number) {
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
}

function VolumeSlider({ value, onChange }: { value: number; onChange: (v: number) => void }) {
    const ref = useRef<HTMLDivElement>(null);
    const dragging = useRef(false);

    const getVal = (e: MouseEvent | React.MouseEvent) => {
        const el = ref.current!;
        const rect = el.getBoundingClientRect();
        return Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    };

    const handleMouseDown = (e: React.MouseEvent) => {
        dragging.current = true;
        onChange(getVal(e));
        e.preventDefault();
    };

    useEffect(() => {
        const move = (e: MouseEvent) => { if (dragging.current) onChange(getVal(e)); };
        const up = () => { dragging.current = false; };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
        return () => { window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", up); };
    });

    return (
        <div
            ref={ref}
            className="slider-root"
            style={{ height: 16, cursor: "pointer" }}
            onMouseDown={handleMouseDown}
            role="slider"
            aria-valuenow={Math.round(value * 100)}
            aria-valuemin={0}
            aria-valuemax={100}
        >
            <div className="slider-track" style={{ height: 4 }}>
                <div className="slider-range" style={{ width: `${value * 100}%` }} />
            </div>
            <div
                className="slider-thumb"
                style={{ position: "absolute", left: `${value * 100}%`, transform: "translateX(-50%)" }}
            />
        </div>
    );
}

export function DeckPanel({ deckId, label, accentColor = "#f59e0b", isOnAir = false }: Props) {
    const [deckState, setDeckState] = useState<DeckStateEvent | null>(null);
    const [vuData, setVuData] = useState<VuEvent | null>(null);
    const [volume, setVolume] = useState(1.0);
    const [monitorMode, setMonitorMode] = useState<"air" | "cue">("air");
    const [waveformData] = useState<Float32Array | null>(null); // loaded separately
    const fileInputRef = useRef<HTMLInputElement>(null);

    const isPlaying = deckState?.state === "Playing" || deckState?.state === "Crossfading";
    const positionMs = deckState?.position_ms ?? 0;
    const durationMs = deckState?.duration_ms ?? 0;
    const remaining = Math.max(0, durationMs - positionMs);

    useEffect(() => {
        const unsub = onDeckStateChanged((e) => {
            if (e.deck === deckId) setDeckState(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [deckId]);

    useEffect(() => {
        const unsub = onVuMeter((e) => {
            if (e.channel === deckId) setVuData(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [deckId]);

    const handleVolumeChange = useCallback((v: number) => {
        setVolume(v);
        setChannelGain(deckId, v).catch(console.error);
    }, [deckId]);

    const handleSeek = useCallback((ms: number) => {
        seekDeck(deckId, ms).catch(console.error);
    }, [deckId]);

    const handlePlay = async () => {
        try {
            if (isPlaying) await pauseDeck(deckId);
            else await playDeck(deckId);
        } catch (e) { console.error(e); }
    };

    const handleStop = async () => {
        try {
            await seekDeck(deckId, 0);
            await pauseDeck(deckId);
        } catch (e) { console.error(e); }
    };

    const handleLoadFile = () => fileInputRef.current?.click();

    const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        try {
            await loadTrack(deckId, (file as any).path ?? file.name);
        } catch (err) { console.error(err); }
        e.target.value = "";
    };

    return (
        <div
            className="deck-panel"
            style={{
                borderColor: isOnAir ? accentColor + "70" : "var(--border-default)",
                boxShadow: isOnAir ? `0 0 24px ${accentColor}20` : "none",
                flex: 1,
            }}
        >
            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <div
                        style={{
                            width: 6, height: 6, borderRadius: "50%",
                            background: isPlaying ? accentColor : "var(--text-muted)",
                            boxShadow: isPlaying ? `0 0 8px ${accentColor}` : "none",
                            transition: "all 0.2s",
                        }}
                    />
                    <span
                        className="font-semibold tracking-widest uppercase"
                        style={{ fontSize: 11, color: isPlaying ? accentColor : "var(--text-secondary)" }}
                    >
                        {label}
                    </span>
                    {isOnAir && (
                        <span className="badge badge-on-air" style={{ fontSize: 8 }}>LIVE</span>
                    )}
                </div>
                <div className="flex items-center gap-1">
                    <button
                        className="btn btn-ghost"
                        style={{ padding: "3px 8px", fontSize: 10 }}
                        onClick={handleLoadFile}
                        title="Load track"
                    >
                        <Music2 size={11} />
                        LOAD
                    </button>
                    <input
                        ref={fileInputRef}
                        type="file"
                        accept="audio/*"
                        style={{ display: "none" }}
                        onChange={handleFileChange}
                    />
                </div>
            </div>

            {/* Track Info */}
            <div
                style={{
                    background: "var(--bg-input)",
                    border: "1px solid var(--border-default)",
                    borderRadius: "var(--r-md)",
                    padding: "8px 10px",
                    minHeight: 48,
                }}
            >
                {deckState && deckState.state !== "Idle" ? (
                    <div>
                        <div className="font-medium" style={{ fontSize: 12, color: "var(--text-primary)", lineHeight: 1.3 }}>
                            Now Loading...
                        </div>
                        <div className="text-xs text-muted" style={{ marginTop: 2 }}>
                            {deckId.replace("_", " ").toUpperCase()}
                        </div>
                    </div>
                ) : (
                    <div className="flex items-center gap-2 text-muted" style={{ height: 32 }}>
                        <Music2 size={14} />
                        <span style={{ fontSize: 11 }}>No track loaded — click LOAD to browse</span>
                    </div>
                )}
            </div>

            {/* Waveform */}
            <WaveformCanvas
                waveformData={waveformData}
                positionMs={positionMs}
                durationMs={durationMs}
                onSeek={handleSeek}
                height={52}
                color={accentColor}
            />

            {/* Time display */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-1">
                    <span className="mono" style={{ fontSize: 22, fontWeight: 600, color: "var(--text-primary)", letterSpacing: "0.04em" }}>
                        {formatTime(positionMs)}
                    </span>
                    <span className="text-muted" style={{ fontSize: 11, marginTop: 4 }}>pos</span>
                </div>
                <div className="flex items-center gap-1">
                    <span className="text-muted" style={{ fontSize: 11, marginTop: 4 }}>rem</span>
                    <span className="mono font-medium" style={{ fontSize: 16, color: remaining < 30000 && remaining > 0 ? "#ef4444" : "var(--text-secondary)", letterSpacing: "0.04em" }}>
                        −{formatTime(remaining)}
                    </span>
                </div>
                <span className="mono text-muted" style={{ fontSize: 11 }}>
                    {formatTime(durationMs)}
                </span>
            </div>

            {/* Transport */}
            <div className="flex items-center gap-2">
                <button className="transport-btn" onClick={() => seekDeck(deckId, 0)} title="Return to start">
                    <SkipBack size={13} />
                </button>
                <button
                    className={`transport-btn play ${isPlaying ? "stop" : ""}`}
                    onClick={handlePlay}
                    title={isPlaying ? "Pause" : "Play"}
                    style={isPlaying ? {
                        background: "var(--red-glow)",
                        borderColor: "var(--red-dim)",
                        color: "var(--red)",
                        width: 38, height: 38,
                    } : {}}
                >
                    {isPlaying ? <Pause size={16} /> : <Play size={16} />}
                </button>
                <button className="transport-btn stop" onClick={handleStop} title="Stop">
                    <Square size={13} />
                </button>
                <button className="transport-btn" title="Next cue">
                    <SkipForward size={13} />
                </button>

                <div style={{ flex: 1 }} />

                {/* VU */}
                <VUMeter vuData={vuData} height={32} width={24} compact />
            </div>

            {/* Volume + Monitor */}
            <div className="flex items-center gap-3">
                <Volume2 size={12} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                <div style={{ flex: 1 }}>
                    <VolumeSlider value={volume} onChange={handleVolumeChange} />
                </div>
                <span className="mono" style={{ fontSize: 10, color: accentColor, minWidth: 28 }}>
                    {Math.round(volume * 100)}%
                </span>

                {/* Air / Cue toggle */}
                <div className="flex" style={{ border: "1px solid var(--border-strong)", borderRadius: "var(--r-md)", overflow: "hidden" }}>
                    <button
                        onClick={() => setMonitorMode("air")}
                        style={{
                            padding: "4px 8px",
                            fontSize: 10,
                            fontWeight: 600,
                            letterSpacing: "0.08em",
                            border: "none",
                            cursor: "pointer",
                            background: monitorMode === "air" ? accentColor : "var(--bg-input)",
                            color: monitorMode === "air" ? "#000" : "var(--text-muted)",
                            transition: "all 0.15s",
                        }}
                    >
                        <Radio size={10} style={{ display: "inline", marginRight: 3 }} />AIR
                    </button>
                    <button
                        onClick={() => setMonitorMode("cue")}
                        style={{
                            padding: "4px 8px",
                            fontSize: 10,
                            fontWeight: 600,
                            letterSpacing: "0.08em",
                            border: "none",
                            borderLeft: "1px solid var(--border-strong)",
                            cursor: "pointer",
                            background: monitorMode === "cue" ? "var(--cyan)" : "var(--bg-input)",
                            color: monitorMode === "cue" ? "#000" : "var(--text-muted)",
                            transition: "all 0.15s",
                        }}
                    >
                        <Headphones size={10} style={{ display: "inline", marginRight: 3 }} />CUE
                    </button>
                </div>
            </div>
        </div>
    );
}
