import { useState, useEffect } from "react";
import { Wifi, Settings2, ChartBar, Bot, X, Code, Cloud, BarChart3, Radio, Cog, LayoutTemplate } from "lucide-react";
import { TopBar } from "./TopBar";
import { SourceRow } from "./SourceRow";
import { BottomPanel } from "./BottomPanel";
import { DeckPanel } from "../deck/DeckPanel";
import { DeckWaveformStack } from "../deck/DeckWaveformStack";
import { CrossfadeBar } from "../crossfade/CrossfadeBar";
import { AudioPipelineDiagram } from "../pipeline/AudioPipelineDiagram";
import { ChannelDspDialog } from "../dsp/ChannelDspDialog";
import { CrossfadeSettingsDialog } from "../crossfade/CrossfadeSettingsDialog";
import SchedulerPage from "../automation/SchedulerPage";
import {
    startCrossfade,
    stopStream,
    getStreamStatus,
    onStreamConnected,
    onStreamDisconnected,
    getDeckState,
} from "../../lib/bridge";
import type { DeckId } from "../../lib/bridge";
import { ScriptingPage } from "../../pages/ScriptingPage";
import { GatewayPage } from "../../pages/GatewayPage";
import { AnalyticsPage } from "../../pages/AnalyticsPage";
import StreamingPage from "../../pages/StreamingPage";
import { SettingsPage } from "../../pages/SettingsPage";
import { VoiceFXStrip } from "../voice/VoiceFXStrip";
import { MicSettings } from "../voice/MicSettings";
import { VoiceTrackRecorder } from "../voice/VoiceTrackRecorder";

type PipelineTarget = { channel: DeckId | "master"; label: string; stage: string } | null;
type LayoutState = {
    deckA: boolean;
    deckB: boolean;
    xfade: boolean;
    sources: boolean;
    sourceAux1: boolean;
    sourceAux2: boolean;
    sourceSfx: boolean;
    sourceVoiceFx: boolean;
    voiceStrip: boolean;
};

const DEFAULT_LAYOUT: LayoutState = {
    deckA: true,
    deckB: true,
    xfade: true,
    sources: true,
    sourceAux1: true,
    sourceAux2: true,
    sourceSfx: true,
    sourceVoiceFx: true,
    voiceStrip: true,
};

const SOURCE_LAYOUT_KEY_BY_DECK: Record<"aux_1" | "aux_2" | "sound_fx" | "voice_fx", keyof LayoutState> = {
    aux_1: "sourceAux1",
    aux_2: "sourceAux2",
    sound_fx: "sourceSfx",
    voice_fx: "sourceVoiceFx",
};

function normalizeLayoutState(raw: unknown): LayoutState {
    if (!raw || typeof raw !== "object") {
        return { ...DEFAULT_LAYOUT };
    }
    const obj = raw as Record<string, unknown>;
    const boolOr = (key: keyof LayoutState) =>
        typeof obj[key] === "boolean" ? (obj[key] as boolean) : DEFAULT_LAYOUT[key];
    return {
        deckA: boolOr("deckA"),
        deckB: boolOr("deckB"),
        xfade: boolOr("xfade"),
        sources: boolOr("sources"),
        sourceAux1: boolOr("sourceAux1"),
        sourceAux2: boolOr("sourceAux2"),
        sourceSfx: boolOr("sourceSfx"),
        sourceVoiceFx: boolOr("sourceVoiceFx"),
        voiceStrip: boolOr("voiceStrip"),
    };
}


export function MainWindow() {
    const [isOnAir, setIsOnAir] = useState(false);
    const [streamConnected, setStreamConnected] = useState(false);
    const [showPipeline, setShowPipeline] = useState(false);
    const [showScheduler, setShowScheduler] = useState(false);
    const [showScripting, setShowScripting] = useState(false);
    const [showGateway, setShowGateway] = useState(false);
    const [showAnalytics, setShowAnalytics] = useState(false);
    const [showStreaming, setShowStreaming] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [showMicSettings, setShowMicSettings] = useState(false);
    const [showVtRecorder, setShowVtRecorder] = useState(false);
    const [dspTarget, setDspTarget] = useState<PipelineTarget>(null);

    // ── Hideable-panel layout (persisted to localStorage) ─────────────────
    const [layout, setLayout] = useState<LayoutState>(() => {
        try {
            const saved = localStorage.getItem("dz-layout");
            return normalizeLayoutState(saved ? JSON.parse(saved) : null);
        } catch {
            return { ...DEFAULT_LAYOUT };
        }
    });
    const [showLayoutMenu, setShowLayoutMenu] = useState(false);

    useEffect(() => {
        localStorage.setItem("dz-layout", JSON.stringify(layout));
    }, [layout]);

    const toggleLayout = (key: keyof LayoutState) =>
        setLayout((l) => ({ ...l, [key]: !l[key] }));

    const toggleSourceVisibility = (deck: "aux_1" | "aux_2" | "sound_fx" | "voice_fx") => {
        const key = SOURCE_LAYOUT_KEY_BY_DECK[deck];
        setLayout((l) => ({ ...l, [key]: !l[key] }));
    };

    // Stream status
    useEffect(() => {
        getStreamStatus().then(setStreamConnected).catch(() => { });
        const unsubCon = onStreamConnected(() => setStreamConnected(true));
        const unsubDis = onStreamDisconnected(() => setStreamConnected(false));
        return () => {
            unsubCon.then((f) => f());
            unsubDis.then((f) => f());
        };
    }, []);

    // Keyboard shortcuts
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
            if (e.key === "F1") setShowPipeline((v) => !v);
            if (e.key === "F5") {
                e.preventDefault();
                setIsOnAir((v) => !v);
            }
        };
        window.addEventListener("keydown", handler);
        return () => window.removeEventListener("keydown", handler);
    }, []);

    const handleForceCrossfade = async () => {
        try {
            const [a, b] = await Promise.all([getDeckState("deck_a"), getDeckState("deck_b")]);
            const isPlaying = (s?: string) => s === "playing" || s === "crossfading";
            const isLoaded = (s?: string) => s === "ready" || s === "paused" || isPlaying(s);

            if (isPlaying(a?.state) && isLoaded(b?.state)) {
                await startCrossfade("deck_a", "deck_b");
                return;
            }
            if (isPlaying(b?.state) && isLoaded(a?.state)) {
                await startCrossfade("deck_b", "deck_a");
                return;
            }

            // If both are playing, fade out the deck closer to its end.
            if (isPlaying(a?.state) && isPlaying(b?.state)) {
                const remA = Math.max(0, (a?.duration_ms ?? 0) - (a?.position_ms ?? 0));
                const remB = Math.max(0, (b?.duration_ms ?? 0) - (b?.position_ms ?? 0));
                if (remA <= remB) {
                    await startCrossfade("deck_a", "deck_b");
                } else {
                    await startCrossfade("deck_b", "deck_a");
                }
                return;
            }

            // Final fallback keeps prior behavior.
            await startCrossfade("deck_a", "deck_b");
        } catch (e) {
            console.error(e);
        }
    };

    const handleStopStream = async () => {
        try {
            await stopStream();
            setStreamConnected(false);
        } catch (e) {
            console.error(e);
        }
    };

    const handlePipelineNodeClick = (stage: string, channel: string) => {
        const labelMap: Record<string, string> = {
            deck_a: "DECK A",
            deck_b: "DECK B",
            sound_fx: "SFX",
            aux_1: "AUX 1",
            aux_2: "AUX 2",
            voice_fx: "VOICE FX",
            master: "MASTER",
        };
        setDspTarget({
            channel: channel as DeckId | "master",
            label: labelMap[channel] ?? channel.toUpperCase(),
            stage,
        });
    };

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                height: "100vh",
                background: "var(--bg-root)",
                overflow: "hidden",
            }}
        >
            {/* Top bar */}
            <TopBar isOnAir={isOnAir} streamConnected={streamConnected} />

            {/* Toolbar row */}
            <div
                className="flex items-center gap-2"
                style={{
                    padding: "6px 12px",
                    borderBottom: "1px solid var(--border-default)",
                    background: "var(--bg-surface)",
                    flexShrink: 0,
                }}
            >
                {/* On Air toggle */}
                <button
                    className="btn"
                    style={{
                        fontSize: 10,
                        fontWeight: 700,
                        letterSpacing: "0.1em",
                        background: isOnAir ? "var(--red)" : "var(--bg-elevated)",
                        borderColor: isOnAir ? "var(--red)" : "var(--border-strong)",
                        color: isOnAir ? "#fff" : "var(--text-muted)",
                    }}
                    onClick={() => setIsOnAir((v) => !v)}
                    title="F5"
                >
                    <div
                        className={isOnAir ? "pulse-dot pulse-dot-red" : ""}
                        style={!isOnAir ? { width: 7, height: 7, borderRadius: "50%", background: "var(--text-muted)" } : {}}
                    />
                    ON AIR
                </button>

                <div style={{ width: 1, height: 20, background: "var(--border-strong)" }} />

                <CrossfadeSettingsDialog
                    trigger={
                        <button className="btn btn-ghost" style={{ fontSize: 10 }}>
                            <Settings2 size={12} />
                            XFade
                        </button>
                    }
                />

                <button
                    className="btn btn-ghost"
                    style={{
                        fontSize: 10,
                        background: showPipeline ? "var(--amber-glow)" : "transparent",
                        borderColor: showPipeline ? "var(--amber-dim)" : "var(--border-default)",
                        color: showPipeline ? "var(--amber)" : "var(--text-muted)",
                    }}
                    onClick={() => setShowPipeline((v) => !v)}
                    title="F1"
                >
                    <ChartBar size={12} />
                    Pipeline
                </button>

                <button
                    className="btn btn-ghost"
                    style={{
                        fontSize: 10,
                        background: showScheduler ? "rgba(139,92,246,.15)" : "transparent",
                        borderColor: showScheduler ? "rgba(139,92,246,.5)" : "var(--border-default)",
                        color: showScheduler ? "#a78bfa" : "var(--text-muted)",
                    }}
                    onClick={() => setShowScheduler((v) => !v)}
                    title="Automation & Scheduling"
                >
                    <Bot size={12} />
                    Automation
                </button>

                <button
                    className="btn btn-ghost"
                    style={{
                        fontSize: 10,
                        background: showGateway ? "rgba(59,130,246,.15)" : "transparent",
                        borderColor: showGateway ? "rgba(59,130,246,.5)" : "var(--border-default)",
                        color: showGateway ? "#3b82f6" : "var(--text-muted)",
                    }}
                    onClick={() => setShowGateway((v) => !v)}
                    title="Gateway"
                >
                    <Cloud size={12} />
                    Gateway
                </button>

                <button
                    className="btn btn-ghost"
                    style={{
                        fontSize: 10,
                        background: showAnalytics ? "rgba(251,146,60,.15)" : "transparent",
                        borderColor: showAnalytics ? "rgba(251,146,60,.5)" : "var(--border-default)",
                        color: showAnalytics ? "#fb923c" : "var(--text-muted)",
                    }}
                    onClick={() => setShowAnalytics((v) => !v)}
                    title="Analytics"
                >
                    <BarChart3 size={12} />
                    Analytics
                </button>

                <button
                    className="btn btn-ghost"
                    style={{
                        fontSize: 10,
                        background: showStreaming ? "rgba(6,182,212,.15)" : "transparent",
                        borderColor: showStreaming ? "rgba(6,182,212,.5)" : "var(--border-default)",
                        color: showStreaming ? "var(--cyan)" : "var(--text-muted)",
                    }}
                    onClick={() => setShowStreaming((v) => !v)}
                    title="Encoders & Streaming"
                >
                    <Radio size={12} />
                    Encoders
                </button>

                <div style={{ marginLeft: "auto" }} />

                {/* Layout toggle button + popover */}
                <div style={{ position: "relative" }}>
                    <button
                        className="btn btn-ghost"
                        style={{
                            fontSize: 10,
                            background: showLayoutMenu ? "rgba(245,158,11,.15)" : "transparent",
                            borderColor: showLayoutMenu ? "var(--amber-dim)" : "var(--border-default)",
                            color: showLayoutMenu ? "var(--amber)" : "var(--text-muted)",
                        }}
                        onClick={() => setShowLayoutMenu((v) => !v)}
                        title="Toggle panel visibility"
                    >
                        <LayoutTemplate size={12} />
                        Layout
                    </button>

                    {showLayoutMenu && (
                        <div
                            style={{
                                position: "absolute",
                                top: "calc(100% + 4px)",
                                right: 0,
                                zIndex: 200,
                                background: "var(--bg-elevated)",
                                border: "1px solid var(--border-strong)",
                                borderRadius: "var(--r-md)",
                                padding: "8px 10px",
                                minWidth: 140,
                                display: "flex",
                                flexDirection: "column",
                                gap: 6,
                                boxShadow: "0 4px 20px rgba(0,0,0,0.4)",
                            }}
                            onMouseLeave={() => setShowLayoutMenu(false)}
                        >
                            <span className="section-label" style={{ marginBottom: 2 }}>Visible Panels</span>
                            {(
                                [
                                    { key: "deckA", label: "Deck A" },
                                    { key: "deckB", label: "Deck B" },
                                    { key: "xfade", label: "Crossfade" },
                                    { key: "sources", label: "Sources Row" },
                                    { key: "sourceAux1", label: "AUX 1" },
                                    { key: "sourceAux2", label: "AUX 2" },
                                    { key: "sourceSfx", label: "SFX" },
                                    { key: "sourceVoiceFx", label: "Voice FX Deck" },
                                    { key: "voiceStrip", label: "Voice Strip" },
                                ] as { key: keyof LayoutState; label: string }[]
                            ).map(({ key, label }) => (
                                <label
                                    key={key}
                                    className="flex items-center gap-2"
                                    style={{ cursor: "pointer", userSelect: "none" }}
                                >
                                    <input
                                        type="checkbox"
                                        checked={layout[key]}
                                        onChange={() => toggleLayout(key)}
                                        style={{ accentColor: "var(--amber)", width: 12, height: 12 }}
                                    />
                                    <span style={{ fontSize: 11, color: "var(--text-primary)" }}>{label}</span>
                                </label>
                            ))}
                        </div>
                    )}
                </div>

                <button
                    className="btn btn-ghost"
                    style={{
                        fontSize: 10,
                        background: showSettings ? "rgba(148,163,184,.15)" : "transparent",
                        borderColor: showSettings ? "rgba(148,163,184,.4)" : "var(--border-default)",
                        color: showSettings ? "#94a3b8" : "var(--text-muted)",
                    }}
                    onClick={() => setShowSettings((v) => !v)}
                    title="Settings"
                >
                    <Cog size={12} />
                    Settings
                </button>

                {/* Stream status indicator */}
                {streamConnected && (
                    <button
                        className="btn btn-danger"
                        style={{ fontSize: 10 }}
                        onClick={handleStopStream}
                    >
                        <Wifi size={12} />
                        Stop Stream
                    </button>
                )}
            </div>

            {/* Main area */}
            <div
                style={{
                    flex: 1,
                    display: "flex",
                    flexDirection: "column",
                    overflow: "hidden",
                    padding: "10px 12px",
                    gap: 10,
                    minHeight: 0,
                }}
            >
                {/* Pipeline diagram (collapsible) */}
                {showPipeline && (
                    <div
                        style={{
                            background: "var(--bg-panel)",
                            border: "1px solid var(--border-default)",
                            borderRadius: "var(--r-lg)",
                            padding: 12,
                            flexShrink: 0,
                        }}
                    >
                        <div className="section-label" style={{ marginBottom: 8 }}>Audio Mixer Pipeline</div>
                        <AudioPipelineDiagram
                            onNodeClick={handlePipelineNodeClick}
                            activeChannels={["deck_a"]}
                        />
                    </div>
                )}

                {/* Deck area + Crossfade — conditionally rendered */}
                {(layout.deckA || layout.deckB) ? (
                    <div
                        style={{
                            display: "flex",
                            flexDirection: "column",
                            gap: 10,
                            flexShrink: 0,
                        }}
                    >
                        {(layout.deckA || layout.deckB) && (
                            <DeckWaveformStack showDeckA={layout.deckA} showDeckB={layout.deckB} />
                        )}

                        <div
                            style={{
                                display: "flex",
                                gap: 10,
                                flexShrink: 0,
                            }}
                        >
                            {layout.deckA && (
                                <DeckPanel
                                    deckId="deck_a"
                                    label="DECK A"
                                    accentColor="#f59e0b"
                                    isOnAir={isOnAir}
                                    onCollapse={() => toggleLayout("deckA")}
                                />
                            )}

                            {layout.xfade && layout.deckA && layout.deckB && (
                                <CrossfadeBar
                                    deckA={{ label: "A" }}
                                    deckB={{ label: "B" }}
                                    onForceCrossfade={handleForceCrossfade}
                                />
                            )}

                            {layout.deckB && (
                                <DeckPanel
                                    deckId="deck_b"
                                    label="DECK B"
                                    accentColor="#06b6d4"
                                    onCollapse={() => toggleLayout("deckB")}
                                />
                            )}
                        </div>
                    </div>
                ) : (
                    /* Restore bar — shown when both decks are hidden */
                    <div
                        style={{
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            gap: 8,
                            padding: "8px 12px",
                            background: "var(--bg-elevated)",
                            border: "1px dashed var(--border-strong)",
                            borderRadius: "var(--r-md)",
                            flexShrink: 0,
                            cursor: "pointer",
                        }}
                        onClick={() => setLayout((l) => ({ ...l, deckA: true, deckB: true, xfade: true }))}
                    >
                        <LayoutTemplate size={12} style={{ color: "var(--text-muted)" }} />
                        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                            Decks hidden — click to restore
                        </span>
                    </div>
                )}

                {layout.voiceStrip && (
                    <VoiceFXStrip
                        onOpenSettings={() => setShowMicSettings(true)}
                        onOpenVtRecorder={() => setShowVtRecorder(true)}
                    />
                )}

                {/* AUX / SFX / Voice row */}
                {layout.sources && (
                    <SourceRow
                        visibleSourceIds={[
                            ...(layout.sourceAux1 ? (["aux_1"] as const) : []),
                            ...(layout.sourceAux2 ? (["aux_2"] as const) : []),
                            ...(layout.sourceSfx ? (["sound_fx"] as const) : []),
                            ...(layout.sourceVoiceFx ? (["voice_fx"] as const) : []),
                        ]}
                        onToggleSource={toggleSourceVisibility}
                        onShowAllSources={() =>
                            setLayout((l) => ({
                                ...l,
                                sourceAux1: true,
                                sourceAux2: true,
                                sourceSfx: true,
                                sourceVoiceFx: true,
                            }))
                        }
                    />
                )}

                {/* Bottom panel: Queue | Library | Requests | History | Logs */}
                <BottomPanel />
            </div>

            {/* DSP dialog (opened from pipeline diagram node click) */}
            {dspTarget && (
                <ChannelDspDialog
                    channel={dspTarget.channel}
                    channelLabel={`${dspTarget.label} — ${dspTarget.stage.toUpperCase()}`}
                    trigger={
                        <button style={{ display: "none" }} id="__dsp-trigger-hidden" />
                    }
                />
            )}

            {/* Mic settings */}
            {showMicSettings && <MicSettings onClose={() => setShowMicSettings(false)} />}

            {/* Voice Track Recorder */}
            {showVtRecorder && <VoiceTrackRecorder onClose={() => setShowVtRecorder(false)} />}

            {/* Scheduler overlay */}
            {showScheduler && (
                <div
                    style={{
                        position: "fixed", inset: 0,
                        background: "rgba(0,0,0,0.65)",
                        zIndex: 150,
                        backdropFilter: "blur(4px)",
                        display: "flex",
                        alignItems: "stretch",
                    }}
                    onClick={() => setShowScheduler(false)}
                >
                    <div
                        style={{
                            position: "absolute",
                            top: 0, right: 0, bottom: 0,
                            width: "min(780px, 90vw)",
                            background: "var(--bg-panel)",
                            borderLeft: "1px solid var(--border-strong)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            animation: "slideInRight 200ms ease",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                padding: "10px 14px",
                                borderBottom: "1px solid var(--border-default)",
                                flexShrink: 0,
                            }}
                        >
                            <Bot size={15} style={{ color: "#a78bfa" }} />
                            <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                                Automation & Scheduling
                            </span>
                            <button
                                onClick={() => setShowScheduler(false)}
                                style={{
                                    marginLeft: "auto",
                                    background: "none", border: "none",
                                    color: "var(--text-dim)", cursor: "pointer",
                                }}
                            >
                                <X size={16} />
                            </button>
                        </div>
                        <div style={{ flex: 1, overflow: "hidden" }}>
                            <SchedulerPage />
                        </div>
                    </div>
                </div>
            )}

            {/* Scripting overlay */}
            {showScripting && (
                <div
                    style={{
                        position: "fixed", inset: 0,
                        background: "rgba(0,0,0,0.65)",
                        zIndex: 151,
                        backdropFilter: "blur(4px)",
                        display: "flex",
                        alignItems: "stretch",
                    }}
                    onClick={() => setShowScripting(false)}
                >
                    <div
                        style={{
                            position: "absolute",
                            top: 0, right: 0, bottom: 0,
                            width: "min(780px, 90vw)",
                            background: "var(--bg-panel)",
                            borderLeft: "1px solid var(--border-strong)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            animation: "slideInRight 200ms ease",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                padding: "10px 14px",
                                borderBottom: "1px solid var(--border-default)",
                                flexShrink: 0,
                            }}
                        >
                            <Code size={15} style={{ color: "#10b981" }} />
                            <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                                Scripting Engine
                            </span>
                            <button
                                onClick={() => setShowScripting(false)}
                                style={{
                                    marginLeft: "auto",
                                    background: "none", border: "none",
                                    color: "var(--text-dim)", cursor: "pointer",
                                }}
                            >
                                <X size={16} />
                            </button>
                        </div>
                        <div style={{ flex: 1, overflow: "hidden" }}>
                            <ScriptingPage />
                        </div>
                    </div>
                </div>
            )}

            {/* Gateway page modal */}
            {showGateway && (
                <div
                    style={{
                        position: "fixed", inset: 0,
                        background: "rgba(0,0,0,0.65)",
                        zIndex: 151,
                        backdropFilter: "blur(4px)",
                        display: "flex",
                        alignItems: "stretch",
                    }}
                    onClick={() => setShowGateway(false)}
                >
                    <div
                        style={{
                            position: "absolute",
                            top: 0, right: 0, bottom: 0,
                            width: "min(900px, 95vw)",
                            background: "var(--bg-panel)",
                            borderLeft: "1px solid var(--border-strong)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            animation: "slideInRight 200ms ease",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                padding: "10px 14px",
                                borderBottom: "1px solid var(--border-default)",
                                flexShrink: 0,
                            }}
                        >
                            <Cloud size={15} style={{ color: "#3b82f6" }} />
                            <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                                DBE Gateway
                            </span>
                            <button
                                onClick={() => setShowGateway(false)}
                                style={{
                                    marginLeft: "auto",
                                    background: "none", border: "none",
                                    color: "var(--text-dim)", cursor: "pointer",
                                }}
                            >
                                <X size={16} />
                            </button>
                        </div>
                        <div style={{ flex: 1, overflow: "hidden" }}>
                            <GatewayPage />
                        </div>
                    </div>
                </div>
            )}

            {/* Streaming / Encoders overlay */}
            {showStreaming && (
                <div
                    style={{
                        position: "fixed", inset: 0,
                        background: "rgba(0,0,0,0.65)",
                        zIndex: 151,
                        backdropFilter: "blur(4px)",
                        display: "flex",
                        alignItems: "stretch",
                    }}
                    onClick={() => setShowStreaming(false)}
                >
                    <div
                        style={{
                            position: "absolute",
                            top: 0, right: 0, bottom: 0,
                            width: "min(900px, 95vw)",
                            background: "var(--bg-panel)",
                            borderLeft: "1px solid var(--border-strong)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            animation: "slideInRight 200ms ease",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                padding: "10px 14px",
                                borderBottom: "1px solid var(--border-default)",
                                flexShrink: 0,
                            }}
                        >
                            <Radio size={15} style={{ color: "var(--cyan)" }} />
                            <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                                Encoders & Streaming
                            </span>
                            <button
                                onClick={() => setShowStreaming(false)}
                                style={{
                                    marginLeft: "auto",
                                    background: "none", border: "none",
                                    color: "var(--text-dim)", cursor: "pointer",
                                }}
                            >
                                <X size={16} />
                            </button>
                        </div>
                        <div style={{ flex: 1, overflow: "hidden" }}>
                            <StreamingPage />
                        </div>
                    </div>
                </div>
            )}

            {/* Settings overlay */}
            {showSettings && (
                <div
                    style={{
                        position: "fixed", inset: 0,
                        background: "rgba(0,0,0,0.65)",
                        zIndex: 151,
                        backdropFilter: "blur(4px)",
                        display: "flex",
                        alignItems: "stretch",
                    }}
                    onClick={() => setShowSettings(false)}
                >
                    <div
                        style={{
                            position: "absolute",
                            top: 0, right: 0, bottom: 0,
                            width: "min(900px, 95vw)",
                            background: "var(--bg-panel)",
                            borderLeft: "1px solid var(--border-strong)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            animation: "slideInRight 200ms ease",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                padding: "10px 14px",
                                borderBottom: "1px solid var(--border-default)",
                                flexShrink: 0,
                            }}
                        >
                            <Cog size={15} style={{ color: "#94a3b8" }} />
                            <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                                Settings
                            </span>
                            <button
                                onClick={() => setShowSettings(false)}
                                style={{
                                    marginLeft: "auto",
                                    background: "none", border: "none",
                                    color: "var(--text-dim)", cursor: "pointer",
                                }}
                            >
                                <X size={16} />
                            </button>
                        </div>
                        <div style={{ flex: 1, overflow: "hidden" }}>
                            <SettingsPage />
                        </div>
                    </div>
                </div>
            )}

            {/* Analytics page modal */}
            {showAnalytics && (
                <div
                    style={{
                        position: "fixed", inset: 0,
                        background: "rgba(0,0,0,0.65)",
                        zIndex: 151,
                        backdropFilter: "blur(4px)",
                        display: "flex",
                        alignItems: "stretch",
                    }}
                    onClick={() => setShowAnalytics(false)}
                >
                    <div
                        style={{
                            position: "absolute",
                            top: 0, right: 0, bottom: 0,
                            width: "min(900px, 95vw)",
                            background: "var(--bg-panel)",
                            borderLeft: "1px solid var(--border-strong)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            animation: "slideInRight 200ms ease",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                padding: "10px 14px",
                                borderBottom: "1px solid var(--border-default)",
                                flexShrink: 0,
                            }}
                        >
                            <BarChart3 size={15} style={{ color: "#fb923c" }} />
                            <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                                Analytics & Operations
                            </span>
                            <button
                                onClick={() => setShowAnalytics(false)}
                                style={{
                                    marginLeft: "auto",
                                    background: "none", border: "none",
                                    color: "var(--text-dim)", cursor: "pointer",
                                }}
                            >
                                <X size={16} />
                            </button>
                        </div>
                        <div style={{ flex: 1, overflow: "hidden" }}>
                            <AnalyticsPage />
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
