import { useState, useEffect, useRef } from "react";
import { Play, Pause, Square, Volume2, Mic } from "lucide-react";
import {
    playDeck,
    pauseDeck,
    seekDeck,
    setChannelGain,
    setDeckPitch,
    setDeckTempo,
    onDeckStateChanged,
    onVuMeter,
    loadTrack,
    getDeckState,
    getSong,
    DeckId,
    VuEvent,
    DeckStateEvent,
} from "../../lib/bridge";
import type { SamSong } from "../../lib/bridge";
import { writeEventLog } from "../../lib/bridge7";
import { VUMeter } from "../deck/VUMeter";
import { parseSongDragPayload } from "../../lib/songDrag";

interface SourceChannel {
    id: DeckId;
    label: string;
    color: string;
    icon?: React.ReactNode;
}

const SOURCES: SourceChannel[] = [
    { id: "aux_1",    label: "AUX 1",    color: "#22c55e" },
    { id: "aux_2",    label: "AUX 2",    color: "#ec4899" },
    { id: "sound_fx", label: "SFX",      color: "#8b5cf6" },
    { id: "voice_fx", label: "VOICE FX", color: "#f97316", icon: <Mic size={11} /> },
];

function marqueeText(text: string, color: string, title?: string) {
    return (
        <div className="marquee-line" style={{ fontSize: 9, color }} title={title ?? text}>
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

function SourceStrip({ ch }: { ch: SourceChannel }) {
    const [isPlaying, setIsPlaying] = useState(false);
    const [volume, setVolume] = useState(1.0);
    const [pitchPct, setPitchPct] = useState(0);
    const [tempoPct, setTempoPct] = useState(0);
    const [vuData, setVuData] = useState<VuEvent | null>(null);
    const [deckState, setDeckState] = useState<DeckStateEvent | null>(null);
    const [loadedSong, setLoadedSong] = useState<{ artist: string; title: string } | null>(null);
    const [isDragOver, setIsDragOver] = useState(false);
    const [loadError, setLoadError] = useState<string | null>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        // Seed current deck state so metadata appears even if event listener
        // attaches after the initial load.
        getDeckState(ch.id).then((s) => {
            if (!s) return;
            setDeckState(s);
            setIsPlaying(s.state === "playing" || s.state === "crossfading");
            if (typeof s.pitch_pct === "number") setPitchPct(s.pitch_pct);
            if (typeof s.tempo_pct === "number") setTempoPct(s.tempo_pct);
        }).catch(() => {});

        const unsub = onDeckStateChanged((e) => {
            if (e.deck === ch.id) {
                setDeckState(e);
                setIsPlaying(e.state === "playing" || e.state === "crossfading");
                if (typeof e.pitch_pct === "number") setPitchPct(e.pitch_pct);
                if (typeof e.tempo_pct === "number") setTempoPct(e.tempo_pct);
            }
        });
        return () => { unsub.then((f) => f()); };
    }, [ch.id]);

    useEffect(() => {
        const unsub = onVuMeter((e) => {
            if (e.channel === ch.id) setVuData(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [ch.id]);

    useEffect(() => {
        let cancelled = false;
        const songId = deckState?.song_id ?? null;
        const filePath = deckState?.file_path ?? null;
        if (!songId && !filePath) {
            setLoadedSong(null);
            return;
        }
        if (songId) {
            getSong(songId)
                .then((song) => {
                    if (cancelled) return;
                    if (song) {
                        setLoadedSong({ artist: song.artist || "", title: song.title || filenameFromPath(filePath) });
                    } else {
                        setLoadedSong({ artist: "", title: filenameFromPath(filePath) });
                    }
                })
                .catch(() => {
                    if (!cancelled) setLoadedSong({ artist: "", title: filenameFromPath(filePath) });
                });
        } else {
            setLoadedSong({ artist: "", title: filenameFromPath(filePath) });
        }
        return () => {
            cancelled = true;
        };
    }, [deckState?.song_id, deckState?.file_path]);

    const handlePlayPause = async () => {
        try {
            if (isPlaying) await pauseDeck(ch.id);
            else await playDeck(ch.id);
        } catch (e) { console.error(e); }
    };

    const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const v = parseFloat(e.target.value);
        setVolume(v);
        setChannelGain(ch.id, v).catch(console.error);
    };

    const handlePitchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const v = parseFloat(e.target.value);
        setPitchPct(v);
        setDeckPitch(ch.id, v).catch(console.error);
    };

    const handleTempoChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const v = parseFloat(e.target.value);
        setTempoPct(v);
        setDeckTempo(ch.id, v).catch(console.error);
    };

    const handleLoadFile = () => fileInputRef.current?.click();

    const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const filePath = (file as any).path ?? file.name;
        try {
            await loadTrack(ch.id, filePath);
            setLoadError(null);
            setLoadedSong({ artist: "", title: filenameFromPath(filePath) });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            setLoadError(msg);
            setTimeout(() => setLoadError(null), 6000);
        }
        e.target.value = "";
    };

    return (
        <div
            style={{
                display: "flex",
                alignItems: "center",
                gap: 12,
                padding: "8px 12px",
                background: isDragOver ? `${ch.color}10` : "var(--bg-panel)",
                border: isDragOver
                    ? `1px dashed ${ch.color}`
                    : `1px solid ${isPlaying ? ch.color + "50" : "var(--border-default)"}`,
                borderRadius: "var(--r-md)",
                minWidth: 0,
                width: "100%",
                minHeight: 54,
                transition: "border-color 0.15s, background 0.15s",
            }}
            onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; setIsDragOver(true); }}
            onDragLeave={() => setIsDragOver(false)}
            onDrop={async (e) => {
                e.preventDefault();
                setIsDragOver(false);
                const raw = e.dataTransfer.getData("text/plain");
                if (!raw) return;
                const song = parseSongDragPayload(raw) as SamSong | null;
                if (!song) return;
                try {
                    await loadTrack(ch.id, song.filename, song.id);
                    setLoadError(null);
                    setLoadedSong({ artist: song.artist || "", title: song.title || filenameFromPath(song.filename) });
                } catch (err) {
                    const msg = err instanceof Error ? err.message : String(err);
                    setLoadError(msg);
                    setTimeout(() => setLoadError(null), 6000);
                    writeEventLog({
                        level: "error",
                        category: "audio",
                        event: "track_load_failed",
                        message: `Failed to load "${song.artist} – ${song.title}" on ${ch.id}: ${msg}`,
                        deck: ch.id,
                        songId: song.id,
                    }).catch(() => {});
                }
            }}
        >
            {/* Label */}
            <div className="flex items-center gap-1" style={{ minWidth: 68 }}>
                {ch.icon && <span style={{ color: ch.color }}>{ch.icon}</span>}
                <span
                    className="font-semibold tracking-wide uppercase"
                    style={{
                        fontSize: 10,
                        color: loadError ? "#ef4444" : isPlaying ? ch.color : "var(--text-secondary)",
                        letterSpacing: "0.1em",
                    }}
                    title={loadError ?? undefined}
                >
                    {loadError ? "⚠ ERR" : ch.label}
                </span>
            </div>

            <button
                className="btn btn-ghost"
                style={{ fontSize: 9, padding: "2px 7px", minWidth: 44 }}
                onClick={handleLoadFile}
                title={`Load ${ch.label}`}
            >
                LOAD
            </button>
            <input
                ref={fileInputRef}
                type="file"
                accept="audio/*"
                style={{ display: "none" }}
                onChange={handleFileChange}
            />

            {/* Track metadata */}
            <div style={{ minWidth: 100, maxWidth: 220, overflow: "hidden" }}>
                {loadedSong ? (
                    <>
                        {marqueeText(
                            [loadedSong.artist, loadedSong.title].filter(Boolean).join(" - "),
                            "var(--text-primary)"
                        )}
                        {marqueeText(
                            deckState?.file_path ? filenameFromPath(deckState.file_path) : ch.label,
                            "var(--text-muted)"
                        )}
                    </>
                ) : (
                    <span className="text-muted" style={{ fontSize: 9 }}>No track</span>
                )}
            </div>

            {/* Transport */}
            <div className="flex items-center gap-1">
                <button
                    style={{
                        display: "flex", alignItems: "center", justifyContent: "center",
                        width: 26, height: 26, borderRadius: "var(--r-md)",
                        border: `1px solid ${isPlaying ? ch.color + "60" : "var(--border-strong)"}`,
                        background: isPlaying ? `${ch.color}20` : "var(--bg-input)",
                        color: isPlaying ? ch.color : "var(--text-secondary)",
                        cursor: "pointer", transition: "all 0.15s",
                    }}
                    onClick={handlePlayPause}
                >
                    {isPlaying ? <Pause size={11} /> : <Play size={11} />}
                </button>
                <button
                    style={{
                        display: "flex", alignItems: "center", justifyContent: "center",
                        width: 26, height: 26, borderRadius: "var(--r-md)",
                        border: "1px solid var(--border-strong)",
                        background: "var(--bg-input)", color: "var(--text-muted)",
                        cursor: "pointer",
                    }}
                    onClick={() => { seekDeck(ch.id, 0); pauseDeck(ch.id); }}
                >
                    <Square size={11} />
                </button>
            </div>

            {/* Volume */}
            <Volume2 size={11} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
            <input
                type="range"
                min={0}
                max={1.5}
                step={0.01}
                value={volume}
                onChange={handleVolumeChange}
                style={{ flex: 1, accentColor: ch.color, height: 3, cursor: "pointer" }}
            />
            <span
                className="mono"
                style={{ fontSize: 9, color: ch.color, minWidth: 24, textAlign: "right" }}
            >
                {Math.round(volume * 100)}%
            </span>
            <button className="btn btn-ghost btn-icon" style={{ width: 16, height: 16 }} title="Reset volume" onClick={() => { setVolume(1); setChannelGain(ch.id, 1).catch(console.error); }}>↺</button>

            {/* Pitch + Tempo */}
            <div style={{ display: "flex", flexDirection: "column", gap: 2, width: 110, marginLeft: 2 }}>
                <div className="flex items-center gap-1">
                    <span className="mono text-muted" style={{ fontSize: 8, minWidth: 16 }}>P</span>
                    <input
                        type="range"
                        min={-50}
                        max={50}
                        step={0.1}
                        value={pitchPct}
                        onChange={handlePitchChange}
                        style={{ flex: 1, accentColor: ch.color, height: 2 }}
                    />
                    <button className="btn btn-ghost btn-icon" style={{ width: 14, height: 14 }} title="Reset pitch" onClick={() => { setPitchPct(0); setDeckPitch(ch.id, 0).catch(console.error); }}>↺</button>
                </div>
                <div className="flex items-center gap-1">
                    <span className="mono text-muted" style={{ fontSize: 8, minWidth: 16 }}>T</span>
                    <input
                        type="range"
                        min={-50}
                        max={50}
                        step={0.1}
                        value={tempoPct}
                        onChange={handleTempoChange}
                        style={{ flex: 1, accentColor: ch.color, height: 2 }}
                    />
                    <button className="btn btn-ghost btn-icon" style={{ width: 14, height: 14 }} title="Reset tempo" onClick={() => { setTempoPct(0); setDeckTempo(ch.id, 0).catch(console.error); }}>↺</button>
                </div>
            </div>

            {/* VU */}
            <VUMeter vuData={vuData} height={22} width={16} compact orientation="horizontal" />
        </div>
    );
}

export function SourceRow() {
    return (
        <div
            style={{
                display: "grid",
                gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
                gap: 8,
                padding: "6px 0",
                flexShrink: 0,
            }}
        >
            {SOURCES.map((ch) => (
                <SourceStrip key={ch.id} ch={ch} />
            ))}
        </div>
    );
}
