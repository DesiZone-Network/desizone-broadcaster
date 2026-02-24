import { useState, useEffect } from "react";
import { ArrowRight, Zap, Settings2 } from "lucide-react";
import { onCrossfadeProgress, CrossfadeProgressEvent } from "../../lib/bridge";
import { CrossfadeSettingsDialog } from "./CrossfadeSettingsDialog";

interface Props {
    deckA: { label: string };
    deckB: { label: string };
    onForceCrossfade?: () => void;
}

export function CrossfadeBar({ deckA, deckB, onForceCrossfade }: Props) {
    const [progress, setProgress] = useState<CrossfadeProgressEvent | null>(null);
    const [isAuto, setIsAuto] = useState(true);

    useEffect(() => {
        const unsub = onCrossfadeProgress((e) => setProgress(e));
        return () => { unsub.then((f) => f()); };
    }, []);

    const pct = progress?.progress ?? 0.5;
    const isActive = progress !== null && pct > 0 && pct < 1;

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                gap: 10,
                padding: "10px 14px",
                background: "var(--bg-surface)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--r-lg)",
                minWidth: 220,
                maxWidth: 320,
                flex: 1,
            }}
        >
            {/* Header */}
            <div className="flex items-center justify-between">
                <span
                    className="section-label"
                    style={{ color: isActive ? "var(--amber)" : "var(--text-muted)" }}
                >
                    {isActive ? "CROSSFADING" : "CROSSFADE"}
                </span>
                <div className="flex items-center gap-1">
                    {/* Auto / Manual toggle */}
                    <button
                        className="btn btn-ghost"
                        style={{
                            padding: "2px 8px",
                            fontSize: 9,
                            letterSpacing: "0.1em",
                            background: isAuto ? "var(--amber-glow)" : "transparent",
                            borderColor: isAuto ? "var(--amber-dim)" : "var(--border-default)",
                            color: isAuto ? "var(--amber)" : "var(--text-muted)",
                        }}
                        onClick={() => setIsAuto(!isAuto)}
                    >
                        {isAuto ? "AUTO" : "MAN"}
                    </button>
                    <CrossfadeSettingsDialog
                        trigger={
                            <button className="btn btn-ghost btn-icon" title="Crossfade settings" style={{ width: 24, height: 24 }}>
                                <Settings2 size={12} />
                            </button>
                        }
                    />
                </div>
            </div>

            {/* Crossfade slider bar */}
            <div style={{ position: "relative" }}>
                <div className="xfade-bar-wrap" style={{ height: 8 }}>
                    <div
                        className="xfade-bar-left"
                        style={{ width: `${(1 - pct) * 100}%`, opacity: isActive ? 1 : 0.3 }}
                    />
                    <div
                        className="xfade-bar-right"
                        style={{ width: `${pct * 100}%`, opacity: isActive ? 1 : 0.3 }}
                    />
                    <div
                        className="xfade-handle"
                        style={{
                            left: `${pct * 100}%`,
                            borderColor: isActive ? "var(--text-primary)" : "var(--border-strong)",
                        }}
                    />
                </div>

                {/* Progress text */}
                {isActive && (
                    <div
                        className="mono"
                        style={{
                            textAlign: "center",
                            marginTop: 6,
                            fontSize: 10,
                            color: "var(--amber)",
                        }}
                    >
                        {Math.round(pct * 100)}%
                    </div>
                )}
            </div>

            {/* Deck labels */}
            <div className="flex items-center justify-between">
                <span
                    className="mono font-semibold"
                    style={{
                        fontSize: 10,
                        letterSpacing: "0.1em",
                        color: pct < 0.5 || !isActive ? "var(--amber)" : "var(--text-muted)",
                    }}
                >
                    {deckA.label}
                </span>
                <div className="flex items-center gap-1" style={{ color: "var(--text-muted)" }}>
                    <ArrowRight size={12} />
                </div>
                <span
                    className="mono font-semibold"
                    style={{
                        fontSize: 10,
                        letterSpacing: "0.1em",
                        color: pct > 0.5 && isActive ? "var(--cyan)" : "var(--text-muted)",
                    }}
                >
                    {deckB.label}
                </span>
            </div>

            {/* Force crossfade */}
            <button
                className="btn btn-ghost"
                style={{
                    fontSize: 10,
                    letterSpacing: "0.1em",
                    textTransform: "uppercase",
                    justifyContent: "center",
                    borderColor: "var(--red-dim)",
                    color: "var(--red)",
                    background: "var(--red-glow)",
                }}
                onClick={onForceCrossfade}
            >
                <Zap size={11} />
                FORCE XFADE
            </button>
        </div>
    );
}
