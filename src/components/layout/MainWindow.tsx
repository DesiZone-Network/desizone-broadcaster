import { useState, useEffect } from "react";
import { Wifi, Settings2, ChartBar, Bot, X } from "lucide-react";
import { TopBar } from "./TopBar";
import { SourceRow } from "./SourceRow";
import { BottomPanel } from "./BottomPanel";
import { DeckPanel } from "../deck/DeckPanel";
import { CrossfadeBar } from "../crossfade/CrossfadeBar";
import { AudioPipelineDiagram } from "../pipeline/AudioPipelineDiagram";
import { ChannelDspDialog } from "../dsp/ChannelDspDialog";
import { CrossfadeSettingsDialog } from "../crossfade/CrossfadeSettingsDialog";
import SchedulerPage from "../automation/SchedulerPage";
import { startCrossfade, startStream, stopStream, getStreamStatus, onStreamConnected, onStreamDisconnected } from "../../lib/bridge";
import type { DeckId } from "../../lib/bridge";
import { ScriptingPage } from "../../pages/ScriptingPage";
import { VoiceFXStrip } from "../voice/VoiceFXStrip";
import { MicSettings } from "../voice/MicSettings";
import { VoiceTrackRecorder } from "../voice/VoiceTrackRecorder";
import { Code } from "lucide-react";

type PipelineTarget = { channel: DeckId | "master"; label: string; stage: string } | null;

function StreamDialog({ onClose }: { onClose: () => void }) {
    const [host, setHost] = useState("localhost");
    const [port, setPort] = useState(8000);
    const [mount, setMount] = useState("/stream");
    const [password, setPassword] = useState("hackme");
    const [bitrate, setBitrate] = useState(128);
    const [loading, setLoading] = useState(false);

    const handleStart = async () => {
        setLoading(true);
        try {
            await startStream({ host, port, mount, password, bitrateKbps: bitrate });
            onClose();
        } catch (e) {
            console.error(e);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div
            style={{
                position: "fixed", inset: 0, background: "rgba(0,0,0,0.7)",
                display: "flex", alignItems: "center", justifyContent: "center",
                zIndex: 200, backdropFilter: "blur(4px)",
            }}
            onClick={onClose}
        >
            <div
                style={{
                    background: "var(--bg-elevated)", border: "1px solid var(--border-strong)",
                    borderRadius: "var(--r-xl)", padding: 24, width: 380,
                    animation: "slideIn 160ms ease",
                }}
                onClick={(e) => e.stopPropagation()}
            >
                <div className="font-semibold uppercase tracking-wide" style={{ fontSize: 12, color: "var(--cyan)", marginBottom: 16 }}>
                    Icecast / Shoutcast Stream
                </div>
                {[
                    { label: "Host", value: host, set: setHost, type: "text" },
                    { label: "Port", value: port, set: (v: string) => setPort(parseInt(v)), type: "number" },
                    { label: "Mount", value: mount, set: setMount, type: "text" },
                    { label: "Password", value: password, set: setPassword, type: "password" },
                    { label: "Bitrate (kbps)", value: bitrate, set: (v: string) => setBitrate(parseInt(v)), type: "number" },
                ].map((f) => (
                    <div className="form-row" key={f.label}>
                        <span className="form-label">{f.label}</span>
                        <input
                            type={f.type}
                            className="input"
                            value={f.value}
                            onChange={(e) => (f.set as (v: string) => void)(e.target.value)}
                        />
                    </div>
                ))}
                <div className="flex justify-end gap-2" style={{ marginTop: 20 }}>
                    <button className="btn btn-ghost" onClick={onClose} style={{ fontSize: 11 }}>Cancel</button>
                    <button className="btn btn-primary" onClick={handleStart} disabled={loading} style={{ fontSize: 11 }}>
                        {loading ? "Connecting…" : "Start Stream"}
                    </button>
                </div>
            </div>
        </div>
    );
}

export function MainWindow() {
    const [isOnAir, setIsOnAir] = useState(false);
    const [streamConnected, setStreamConnected] = useState(false);
    const [showPipeline, setShowPipeline] = useState(false);
    const [showStream, setShowStream] = useState(false);
    const [showScheduler, setShowScheduler] = useState(false);
    const [showScripting, setShowScripting] = useState(false);
    const [showMicSettings, setShowMicSettings] = useState(false);
    const [showVtRecorder, setShowVtRecorder] = useState(false);
    const [dspTarget, setDspTarget] = useState<PipelineTarget>(null);

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
                        background: showScripting ? "rgba(16,185,129,.15)" : "transparent",
                        borderColor: showScripting ? "rgba(16,185,129,.5)" : "var(--border-default)",
                        color: showScripting ? "#10b981" : "var(--text-muted)",
                    }}
                    onClick={() => setShowScripting((v) => !v)}
                    title="Scripting"
                >
                    <Code size={12} />
                    Scripting
                </button>

                <div style={{ marginLeft: "auto" }} />

                {/* Stream buttons */}
                {streamConnected ? (
                    <button
                        className="btn btn-danger"
                        style={{ fontSize: 10 }}
                        onClick={handleStopStream}
                    >
                        <Wifi size={12} />
                        Stop Stream
                    </button>
                ) : (
                    <button
                        className="btn btn-ghost"
                        style={{
                            fontSize: 10,
                            borderColor: "var(--cyan-dim)",
                            color: "var(--cyan)",
                            background: "var(--cyan-glow)",
                        }}
                        onClick={() => setShowStream(true)}
                    >
                        <Wifi size={12} />
                        Start Stream
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

                {/* Deck area + Crossfade */}
                <div
                    style={{
                        display: "flex",
                        gap: 10,
                        flexShrink: 0,
                    }}
                >
                    <DeckPanel
                        deckId="deck_a"
                        label="DECK A"
                        accentColor="#f59e0b"
                        isOnAir={isOnAir}
                    />

                    <CrossfadeBar
                        deckA={{ label: "A" }}
                        deckB={{ label: "B" }}
                        onForceCrossfade={handleForceCrossfade}
                    />

                    <DeckPanel
                        deckId="deck_b"
                        label="DECK B"
                        accentColor="#06b6d4"
                    />
                </div>

                <VoiceFXStrip
                    onOpenSettings={() => setShowMicSettings(true)}
                    onOpenVtRecorder={() => setShowVtRecorder(true)}
                />

                {/* AUX / SFX / Voice row */}
                <SourceRow />

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

            {/* Stream dialog */}
            {showStream && <StreamDialog onClose={() => setShowStream(false)} />}

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
        </div>
    );
}
