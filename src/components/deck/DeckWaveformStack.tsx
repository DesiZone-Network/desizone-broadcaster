import { useEffect, useMemo, useState } from "react";
import {
    DeckId,
    DeckStateEvent,
    HotCue,
    BeatGridAnalysis,
    getBeatgrid,
    getDeckState,
    getHotCues,
    getWaveformData,
    onDeckStateChanged,
    seekDeck,
} from "../../lib/bridge";
import { WaveformCanvas } from "./WaveformCanvas";

function formatTime(ms: number) {
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
}

function useDeckWaveform(deckId: DeckId) {
    const [deckState, setDeckState] = useState<DeckStateEvent | null>(null);
    const [waveformData, setWaveformData] = useState<Float32Array | null>(null);
    const [hotCues, setHotCues] = useState<HotCue[]>([]);
    const [beatgrid, setBeatgrid] = useState<BeatGridAnalysis | null>(null);

    useEffect(() => {
        getDeckState(deckId)
            .then((state) => {
                if (state?.deck === deckId) setDeckState(state);
            })
            .catch(() => {});
    }, [deckId]);

    useEffect(() => {
        const unsub = onDeckStateChanged((event) => {
            if (event.deck === deckId) setDeckState(event);
        });
        return () => {
            unsub.then((f) => f()).catch(() => {});
        };
    }, [deckId]);

    useEffect(() => {
        let cancelled = false;
        const songId = deckState?.song_id ?? null;
        const filePath = deckState?.file_path ?? null;

        if (!songId && !filePath) {
            setWaveformData(null);
            setHotCues([]);
            setBeatgrid(null);
            return;
        }

        if (filePath) {
            getWaveformData(filePath, 1800)
                .then((wf) => {
                    if (!cancelled) setWaveformData(wf);
                })
                .catch(() => {
                    if (!cancelled) setWaveformData(null);
                });
        } else {
            setWaveformData(null);
        }

        if (songId) {
            getHotCues(songId)
                .then((cues) => {
                    if (!cancelled) setHotCues(cues);
                })
                .catch(() => {
                    if (!cancelled) setHotCues([]);
                });

            if (filePath) {
                getBeatgrid(songId, filePath)
                    .then((grid) => {
                        if (!cancelled) setBeatgrid(grid);
                    })
                    .catch(() => {
                        if (!cancelled) setBeatgrid(null);
                    });
            } else {
                setBeatgrid(null);
            }
        } else {
            setHotCues([]);
            setBeatgrid(null);
        }

        return () => {
            cancelled = true;
        };
    }, [deckState?.song_id, deckState?.file_path]);

    return { deckState, waveformData, hotCues, beatgrid };
}

function DeckWaveformLane({
    deckId,
    label,
    color,
}: {
    deckId: DeckId;
    label: string;
    color: string;
}) {
    const { deckState, waveformData, hotCues, beatgrid } = useDeckWaveform(deckId);
    const positionMs = deckState?.position_ms ?? 0;
    const durationMs = deckState?.duration_ms ?? 0;
    const remaining = Math.max(0, durationMs - positionMs);
    const stateLabel = deckState?.state ?? "idle";
    const isPlaying = stateLabel === "playing" || stateLabel === "crossfading";
    const playbackRate = deckState?.playback_rate ?? 1;

    const cueMarkers = useMemo(
        () => hotCues.map((cue) => ({ positionMs: cue.position_ms, color: cue.color_hex, label: cue.label })),
        [hotCues]
    );

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                gap: 4,
                background: "var(--bg-input)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--r-md)",
                padding: "6px 8px",
            }}
        >
            <div className="flex items-center justify-between">
                <span className="mono" style={{ fontSize: 10, color, letterSpacing: "0.08em" }}>
                    {label}
                </span>
                <span className="mono text-muted" style={{ fontSize: 10 }}>
                    {durationMs > 0 ? `${formatTime(positionMs)} / -${formatTime(remaining)}` : stateLabel.toUpperCase()}
                </span>
            </div>
            <WaveformCanvas
                waveformData={waveformData}
                positionMs={positionMs}
                durationMs={durationMs}
                isPlaying={isPlaying}
                playbackRate={playbackRate}
                animatePlayhead
                scrollWithPlayhead
                scrollWindowMs={14000}
                onSeek={(ms) => seekDeck(deckId, ms).catch(console.error)}
                cueMarkers={cueMarkers}
                beatGridMs={beatgrid?.beat_times_ms ?? null}
                height={54}
                color={color}
            />
        </div>
    );
}

export function DeckWaveformStack({
    showDeckA = true,
    showDeckB = true,
}: {
    showDeckA?: boolean;
    showDeckB?: boolean;
}) {
    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                gap: 6,
                background: "var(--bg-panel)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--r-lg)",
                padding: "8px 10px",
                flexShrink: 0,
            }}
        >
            {showDeckA && <DeckWaveformLane deckId="deck_a" label="DECK A WAVEFORM" color="#f59e0b" />}
            {showDeckB && <DeckWaveformLane deckId="deck_b" label="DECK B WAVEFORM" color="#06b6d4" />}
        </div>
    );
}
