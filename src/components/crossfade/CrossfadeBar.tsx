import { useEffect, useRef, useState } from "react";
import { ArrowRightLeft, MoveHorizontal, Settings2 } from "lucide-react";
import {
    onCrossfadeProgress,
    CrossfadeProgressEvent,
    setManualCrossfade,
    triggerManualFade,
} from "../../lib/bridge";
import { CrossfadeSettingsDialog } from "./CrossfadeSettingsDialog";

interface Props {
    deckA: { label: string };
    deckB: { label: string };
    onForceCrossfade?: () => void;
}

function ManualSlider({
    value,
    onChange,
}: {
    value: number;
    onChange: (v: number) => void;
}) {
    const ref = useRef<HTMLDivElement>(null);
    const dragging = useRef(false);

    const read = (clientX: number) => {
        const el = ref.current;
        if (!el) return 0;
        const rect = el.getBoundingClientRect();
        const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
        return ratio * 2 - 1;
    };

    useEffect(() => {
        const onMove = (e: MouseEvent) => {
            if (!dragging.current) return;
            onChange(read(e.clientX));
        };
        const onUp = () => {
            dragging.current = false;
        };
        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
        return () => {
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
        };
    }, [onChange]);

    const leftPct = ((value + 1) / 2) * 100;

    return (
        <div
            ref={ref}
            className="xfade-bar-wrap"
            style={{ height: 16, cursor: "ew-resize" }}
            onMouseDown={(e) => {
                dragging.current = true;
                onChange(read(e.clientX));
            }}
            role="slider"
            aria-valuemin={-100}
            aria-valuemax={100}
            aria-valuenow={Math.round(value * 100)}
        >
            <div className="xfade-bar-left" style={{ width: `${100 - leftPct}%`, opacity: 0.8 }} />
            <div className="xfade-bar-right" style={{ width: `${leftPct}%`, opacity: 0.8 }} />
            <div
                className="xfade-handle"
                style={{
                    left: `${leftPct}%`,
                    width: 18,
                    height: 18,
                }}
            />
        </div>
    );
}

export function CrossfadeBar({ deckA, deckB, onForceCrossfade }: Props) {
    const [manualPos, setManualPos] = useState(-1);
    const [progress, setProgress] = useState<CrossfadeProgressEvent | null>(null);
    const [fadeMs, setFadeMs] = useState(8000);

    useEffect(() => {
        const unsub = onCrossfadeProgress((e) => {
            setProgress(e);
            // Keep manual slider visually aligned with timed fades.
            if (e.outgoing_deck === "deck_a" && e.incoming_deck === "deck_b") {
                setManualPos(e.progress * 2 - 1);
            } else if (e.outgoing_deck === "deck_b" && e.incoming_deck === "deck_a") {
                setManualPos(1 - e.progress * 2);
            }
        });
        return () => {
            unsub.then((f) => f());
        };
    }, []);

    const applyManual = (value: number) => {
        const clamped = Math.max(-1, Math.min(1, value));
        setManualPos(clamped);
        setManualCrossfade(clamped).catch(console.error);
    };

    const runTimedFade = (direction: "a_to_b" | "b_to_a") => {
        triggerManualFade(direction, fadeMs).catch(console.error);
        if (direction === "a_to_b") setManualPos(1);
        else setManualPos(-1);
    };

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                gap: 10,
                padding: "10px 12px",
                background: "var(--bg-surface)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--r-lg)",
                minWidth: 240,
                maxWidth: 360,
                flex: 1,
            }}
        >
            <div className="flex items-center justify-between">
                <span className="section-label" style={{ color: "var(--amber)" }}>
                    Fade Control
                </span>
                <CrossfadeSettingsDialog
                    trigger={
                        <button className="btn btn-ghost btn-icon" title="Crossfade settings" style={{ width: 24, height: 24 }}>
                            <Settings2 size={12} />
                        </button>
                    }
                />
            </div>

            <div style={{
                border: "1px solid var(--border-strong)",
                borderRadius: "var(--r-md)",
                background: "var(--bg-input)",
                padding: 10,
            }}>
                <div className="flex items-center justify-between" style={{ marginBottom: 6 }}>
                    <span className="mono" style={{ fontSize: 10, color: "var(--amber)", letterSpacing: "0.08em" }}>{deckA.label}</span>
                    <span className="mono text-muted" style={{ fontSize: 9 }}>{Math.round(manualPos * 100)}%</span>
                    <span className="mono" style={{ fontSize: 10, color: "var(--cyan)", letterSpacing: "0.08em" }}>{deckB.label}</span>
                </div>
                <ManualSlider value={manualPos} onChange={applyManual} />
                <div className="flex items-center justify-between" style={{ marginTop: 6 }}>
                    <button
                        className="btn btn-ghost"
                        style={{ fontSize: 10, padding: "2px 8px" }}
                        onClick={() => applyManual(-1)}
                    >
                        A
                    </button>
                    <MoveHorizontal size={11} style={{ color: "var(--text-muted)" }} />
                    <button
                        className="btn btn-ghost"
                        style={{ fontSize: 10, padding: "2px 8px" }}
                        onClick={() => applyManual(1)}
                    >
                        B
                    </button>
                </div>
            </div>

            <div style={{
                border: "1px solid var(--border-strong)",
                borderRadius: "var(--r-md)",
                background: "var(--bg-input)",
                padding: 10,
                display: "flex",
                flexDirection: "column",
                gap: 8,
            }}>
                <div className="flex items-center justify-between">
                    <span className="section-label">Timed Beat Fade</span>
                    <span className="mono text-muted" style={{ fontSize: 9 }}>{Math.round(fadeMs / 1000)}s</span>
                </div>

                <input
                    type="range"
                    min={1000}
                    max={20000}
                    step={250}
                    value={fadeMs}
                    onChange={(e) => setFadeMs(parseInt(e.target.value, 10))}
                    style={{ width: "100%", accentColor: "var(--amber)", height: 4 }}
                />

                <div className="flex items-center gap-2">
                    <button className="btn btn-ghost" style={{ fontSize: 10, flex: 1 }} onClick={() => runTimedFade("a_to_b")}>
                        A → B
                    </button>
                    <button className="btn btn-ghost" style={{ fontSize: 10, flex: 1 }} onClick={() => runTimedFade("b_to_a")}>
                        B → A
                    </button>
                </div>

                <button
                    className="btn btn-ghost"
                    style={{ fontSize: 10, justifyContent: "center" }}
                    onClick={onForceCrossfade}
                >
                    <ArrowRightLeft size={11} />
                    Force Crossfade
                </button>
            </div>

            {progress && (
                <div className="mono" style={{ fontSize: 10, color: "var(--text-muted)", textAlign: "center" }}>
                    {Math.round(progress.progress * 100)}% • {progress.outgoing_deck.toUpperCase()} → {progress.incoming_deck.toUpperCase()}
                </div>
            )}
        </div>
    );
}
