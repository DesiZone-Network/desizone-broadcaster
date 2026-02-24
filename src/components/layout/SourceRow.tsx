import { useState, useEffect } from "react";
import { Play, Pause, Square, Volume2, Mic } from "lucide-react";
import { playDeck, pauseDeck, seekDeck, setChannelGain, onDeckStateChanged, onVuMeter, DeckId, VuEvent } from "../../lib/bridge";
import { VUMeter } from "../deck/VUMeter";

interface SourceChannel {
    id: DeckId;
    label: string;
    color: string;
    icon?: React.ReactNode;
}

const SOURCES: SourceChannel[] = [
    { id: "aux_1", label: "AUX 1", color: "#22c55e" },
    { id: "sound_fx", label: "SFX", color: "#8b5cf6" },
    { id: "voice_fx", label: "VOICE FX", color: "#f97316", icon: <Mic size={11} /> },
];

function SourceStrip({ ch }: { ch: SourceChannel }) {
    const [isPlaying, setIsPlaying] = useState(false);
    const [volume, setVolume] = useState(1.0);
    const [vuData, setVuData] = useState<VuEvent | null>(null);

    useEffect(() => {
        const unsub = onDeckStateChanged((e) => {
            if (e.deck === ch.id) {
                setIsPlaying(e.state === "Playing");
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

    return (
        <div
            style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "6px 12px",
                background: "var(--bg-panel)",
                border: `1px solid ${isPlaying ? ch.color + "50" : "var(--border-default)"}`,
                borderRadius: "var(--r-md)",
                minWidth: 0,
                flex: 1,
                transition: "border-color 0.2s",
            }}
        >
            {/* Label */}
            <div className="flex items-center gap-1" style={{ minWidth: 68 }}>
                {ch.icon && <span style={{ color: ch.color }}>{ch.icon}</span>}
                <span
                    className="font-semibold tracking-wide uppercase"
                    style={{
                        fontSize: 10,
                        color: isPlaying ? ch.color : "var(--text-secondary)",
                        letterSpacing: "0.1em",
                    }}
                >
                    {ch.label}
                </span>
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
                max={1}
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

            {/* VU */}
            <VUMeter vuData={vuData} height={22} width={16} compact orientation="horizontal" />
        </div>
    );
}

export function SourceRow() {
    return (
        <div
            className="flex items-center gap-3"
            style={{
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
