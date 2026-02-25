import { useState, useEffect, useRef, useCallback } from "react";
import {
    Play, Pause, Square, SkipBack, SkipForward,
    Volume2, Headphones, Radio, Music2, X,
} from "lucide-react";
import {
    playDeck, pauseDeck, seekDeck, setChannelGain,
    setDeckPitch, setDeckTempo,
    onDeckStateChanged, onVuMeter,
    getSong, getWaveformData, loadTrack,
    DeckId, DeckStateEvent, VuEvent,
} from "../../lib/bridge";
import { writeEventLog } from "../../lib/bridge7";
import type { SamSong } from "../../lib/bridge";
import { WaveformCanvas } from "./WaveformCanvas";
import { VUMeter } from "./VUMeter";

interface Props {
    deckId: DeckId;
    label: string;
    accentColor?: string;
    isOnAir?: boolean;
    onCollapse?: () => void;
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

function MarqueeLine({
    text,
    color = "var(--text-primary)",
    fontSize = 12,
    className = "",
}: {
    text: string;
    color?: string;
    fontSize?: number;
    className?: string;
}) {
    const wrapRef = useRef<HTMLDivElement>(null);
    const textRef = useRef<HTMLSpanElement>(null);
    const [overflow, setOverflow] = useState(false);

    useEffect(() => {
        const check = () => {
            const wrap = wrapRef.current;
            const txt = textRef.current;
            if (!wrap || !txt) return;
            setOverflow(txt.scrollWidth > wrap.clientWidth + 4);
        };
        check();
        const id = setTimeout(check, 0);
        window.addEventListener("resize", check);
        return () => {
            clearTimeout(id);
            window.removeEventListener("resize", check);
        };
    }, [text]);

    if (!overflow) {
        return (
            <div
                ref={wrapRef}
                className={className}
                style={{
                    fontSize,
                    color,
                    lineHeight: 1.3,
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                }}
                title={text}
            >
                <span ref={textRef}>{text}</span>
            </div>
        );
    }

    return (
        <div
            ref={wrapRef}
            className={`marquee-line ${className}`}
            style={{ fontSize, color, lineHeight: 1.3 }}
            title={text}
        >
            <span className="marquee-track">{text}</span>
            <span className="marquee-track" aria-hidden="true">{text}</span>
        </div>
    );
}

function filenameFromPath(path?: string | null): string {
    if (!path) return "Track Loaded";
    const base = path.replace(/\\/g, "/").split("/").pop() ?? path;
    return base.replace(/\.[^.]+$/, "");
}

export function DeckPanel({ deckId, label, accentColor = "#f59e0b", isOnAir = false, onCollapse }: Props) {
    const [deckState, setDeckState] = useState<DeckStateEvent | null>(null);
    const [vuData, setVuData] = useState<VuEvent | null>(null);
    const [volume, setVolume] = useState(1.0);
    const [pitchPct, setPitchPct] = useState(0);
    const [tempoPct, setTempoPct] = useState(0);
    const [monitorMode, setMonitorMode] = useState<"air" | "cue">("air");
    const [waveformData, setWaveformData] = useState<Float32Array | null>(null);
    const [isDragOver, setIsDragOver] = useState(false);
    const [loadError, setLoadError] = useState<string | null>(null);
    const [loadedSong, setLoadedSong] = useState<{ title: string; artist: string; path?: string | null } | null>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const loadErrorTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    const isPlaying = deckState?.state === "playing" || deckState?.state === "crossfading";
    const positionMs = deckState?.position_ms ?? 0;
    const durationMs = deckState?.duration_ms ?? 0;
    const remaining = Math.max(0, durationMs - positionMs);
    const headline = loadedSong
        ? [loadedSong.artist, loadedSong.title].filter(Boolean).join(" - ")
        : "";

    useEffect(() => {
        const unsub = onDeckStateChanged((e) => {
            if (e.deck === deckId) setDeckState(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [deckId]);

    useEffect(() => {
        if (!deckState) return;
        if (typeof deckState.pitch_pct === "number") setPitchPct(deckState.pitch_pct);
        if (typeof deckState.tempo_pct === "number") setTempoPct(deckState.tempo_pct);
    }, [deckState?.pitch_pct, deckState?.tempo_pct]);

    useEffect(() => {
        const unsub = onVuMeter((e) => {
            if (e.channel === deckId) setVuData(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [deckId]);

    useEffect(() => {
        let cancelled = false;
        const songId = deckState?.song_id ?? null;
        const filePath = deckState?.file_path ?? null;

        if (!songId && !filePath) {
            setLoadedSong(null);
            setWaveformData(null);
            return;
        }

        if (songId) {
            getSong(songId)
                .then((song) => {
                    if (cancelled || !song) return;
                    setLoadedSong({
                        title: song.title || filenameFromPath(filePath),
                        artist: song.artist || "",
                        path: filePath,
                    });
                })
                .catch(() => {
                    if (cancelled) return;
                    setLoadedSong({
                        title: filenameFromPath(filePath),
                        artist: "",
                        path: filePath,
                    });
                });
        } else {
            setLoadedSong({
                title: filenameFromPath(filePath),
                artist: "",
                path: filePath,
            });
        }

        if (filePath) {
            getWaveformData(filePath, 1400)
                .then((wf) => {
                    if (!cancelled) setWaveformData(wf);
                })
                .catch(() => {
                    if (!cancelled) setWaveformData(null);
                });
        } else {
            setWaveformData(null);
        }

        return () => {
            cancelled = true;
        };
    }, [deckState?.song_id, deckState?.file_path]);

    const handleVolumeChange = useCallback((v: number) => {
        setVolume(v);
        setChannelGain(deckId, v).catch(console.error);
    }, [deckId]);

    const handlePitchChange = useCallback((v: number) => {
        setPitchPct(v);
        setDeckPitch(deckId, v).catch(console.error);
    }, [deckId]);

    const handleTempoChange = useCallback((v: number) => {
        setTempoPct(v);
        setDeckTempo(deckId, v).catch(console.error);
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

    const showLoadError = (msg: string) => {
        setLoadError(msg);
        if (loadErrorTimer.current) clearTimeout(loadErrorTimer.current);
        loadErrorTimer.current = setTimeout(() => setLoadError(null), 6000);
    };

    const handleLoadFile = () => fileInputRef.current?.click();

    const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const filePath = (file as any).path ?? file.name;
        try {
            await loadTrack(deckId, filePath);
            setLoadError(null);
            setLoadedSong({ title: filenameFromPath(filePath), artist: "", path: filePath });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            showLoadError(msg);
            setLoadedSong(null);
            writeEventLog({
                level: "error",
                category: "audio",
                event: "track_load_failed",
                message: `Failed to load file "${file.name}" on ${deckId}: ${msg}`,
                deck: deckId,
            }).catch(() => {});
        }
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
                    {onCollapse && (
                        <button
                            className="btn btn-ghost btn-icon"
                            style={{ width: 20, height: 20, opacity: 0.4 }}
                            onClick={onCollapse}
                            title="Collapse panel"
                        >
                            <X size={11} />
                        </button>
                    )}
                    <input
                        ref={fileInputRef}
                        type="file"
                        accept="audio/*"
                        style={{ display: "none" }}
                        onChange={handleFileChange}
                    />
                </div>
            </div>

            {/* Track Info — also a DnD drop target */}
            <div
                style={{
                    background: isDragOver ? `${accentColor}12` : "var(--bg-input)",
                    border: isDragOver
                        ? `1px dashed ${accentColor}`
                        : "1px solid var(--border-default)",
                    borderRadius: "var(--r-md)",
                    padding: "8px 10px",
                    minHeight: 48,
                    transition: "border-color 0.15s, background 0.15s",
                }}
                onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; setIsDragOver(true); }}
                onDragLeave={() => setIsDragOver(false)}
                onDrop={async (e) => {
                    e.preventDefault();
                    setIsDragOver(false);
                    const raw = e.dataTransfer.getData("text/plain");
                    if (!raw) return;
                    let song: SamSong;
                    try { song = JSON.parse(raw); } catch { return; }
                    try {
                        await loadTrack(deckId, song.filename, song.id);
                        setLoadError(null);
                        setLoadedSong({ title: song.title, artist: song.artist, path: song.filename });
                    } catch (err) {
                        const msg = err instanceof Error ? err.message : String(err);
                        showLoadError(msg);
                        setLoadedSong(null);
                        writeEventLog({
                            level: "error",
                            category: "audio",
                            event: "track_load_failed",
                            message: `Failed to load "${song.artist} – ${song.title}" on ${deckId}: ${msg}`,
                            deck: deckId,
                            songId: song.id,
                        }).catch(() => {});
                    }
                }}
            >
                {loadError ? (
                    <div className="flex items-center gap-2" style={{ minHeight: 32 }}>
                        <span style={{ fontSize: 10, color: "#ef4444", lineHeight: 1.4, wordBreak: "break-all" }}>
                            ⚠ {loadError}
                        </span>
                    </div>
                ) : deckState && deckState.state !== "idle" ? (
                    <div style={{ overflow: "hidden" }}>
                        <MarqueeLine
                            text={
                                headline ||
                                (deckState.state === "playing" || deckState.state === "crossfading"
                                    ? "Playing…"
                                    : "Track Loaded")
                            }
                            className="font-medium"
                            fontSize={12}
                            color="var(--text-primary)"
                        />
                        <MarqueeLine
                            text={loadedSong?.path ? filenameFromPath(loadedSong.path) : deckState.file_path ? filenameFromPath(deckState.file_path) : deckState.state}
                            className="text-xs text-muted"
                            fontSize={10}
                            color="var(--text-muted)"
                        />
                    </div>
                ) : (
                    <div className="flex items-center gap-2 text-muted" style={{ height: 32 }}>
                        <Music2 size={14} />
                        <span style={{ fontSize: 11 }}>
                            {isDragOver ? "Drop to load track" : "No track loaded — drag here or click LOAD"}
                        </span>
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

            {/* Pitch / Tempo (linked playback-rate in this phase) */}
            <div className="flex items-center gap-2" style={{ marginTop: 2 }}>
                <span className="mono text-muted" style={{ fontSize: 9, minWidth: 36 }}>PITCH</span>
                <input
                    type="range"
                    min={-50}
                    max={50}
                    step={0.1}
                    value={pitchPct}
                    onChange={(e) => handlePitchChange(parseFloat(e.target.value))}
                    style={{ flex: 1, accentColor: accentColor, height: 3, cursor: "pointer" }}
                />
                <span className="mono" style={{ fontSize: 9, minWidth: 46, color: "var(--text-secondary)", textAlign: "right" }}>
                    {pitchPct >= 0 ? "+" : ""}{pitchPct.toFixed(1)}%
                </span>
            </div>

            <div className="flex items-center gap-2" style={{ marginTop: 2 }}>
                <span className="mono text-muted" style={{ fontSize: 9, minWidth: 36 }}>TEMPO</span>
                <input
                    type="range"
                    min={-50}
                    max={50}
                    step={0.1}
                    value={tempoPct}
                    onChange={(e) => handleTempoChange(parseFloat(e.target.value))}
                    style={{ flex: 1, accentColor: accentColor, height: 3, cursor: "pointer" }}
                />
                <span className="mono" style={{ fontSize: 9, minWidth: 46, color: "var(--text-secondary)", textAlign: "right" }}>
                    {tempoPct >= 0 ? "+" : ""}{tempoPct.toFixed(1)}%
                </span>
            </div>
        </div>
    );
}
