import { useCallback, useEffect, useState } from "react";
import {
    EncoderConfig,
    EncoderRuntimeState,
    ListenerSnapshot,
    StatsPeriod,
    getEncoders,
    getEncoderRuntime,
    getListenerStats,
    startAllEncoders,
    stopAllEncoders,
    onEncoderStatusChanged,
    onListenerCountUpdated,
} from "../lib/bridge";
import { EncoderList } from "../components/encoders/EncoderList";
import { EncoderStatusCards } from "../components/encoders/EncoderStatusCards";
import { EncoderEditor } from "../components/encoders/EncoderEditor";
import { ListenerGraph } from "../components/stats/ListenerGraph";
import { Radio, Play, Square, RefreshCw } from "lucide-react";

export default function StreamingPage() {
    const [encoders, setEncoders] = useState<EncoderConfig[]>([]);
    const [runtime, setRuntime] = useState<Map<number, EncoderRuntimeState>>(new Map());
    const [snapshots, setSnapshots] = useState<Map<number, ListenerSnapshot[]>>(new Map());
    const [period, setPeriod] = useState<StatsPeriod>("1h");
    const [editTarget, setEditTarget] = useState<EncoderConfig | null | "new">(null);
    const [startingAll, setStartingAll] = useState(false);
    const [stoppingAll, setStoppingAll] = useState(false);

    // ── Data loading ────────────────────────────────────────────────────────────

    const loadEncoders = useCallback(async () => {
        const encs = await getEncoders();
        setEncoders(encs);
    }, []);

    const loadRuntime = useCallback(async () => {
        const states = await getEncoderRuntime();
        const map = new Map<number, EncoderRuntimeState>();
        states.forEach((s) => map.set(s.id, s));
        setRuntime(map);
    }, []);

    const loadSnapshots = useCallback(async (encs: EncoderConfig[], p: StatsPeriod) => {
        const next = new Map<number, ListenerSnapshot[]>();
        await Promise.all(
            encs
                .filter((e) => e.output_type !== "file")
                .map(async (e) => {
                    try {
                        const snaps = await getListenerStats(e.id, p);
                        next.set(e.id, snaps);
                    } catch { /* no-op if DB not yet seeded */ }
                })
        );
        setSnapshots(next);
    }, []);

    // Initial load
    useEffect(() => {
        loadEncoders();
        loadRuntime();
    }, []);

    // Reload snapshots when period or encoders change
    useEffect(() => {
        if (encoders.length > 0) loadSnapshots(encoders, period);
    }, [encoders, period]);

    // Poll runtime state every 5 s
    useEffect(() => {
        const iv = setInterval(loadRuntime, 5000);
        return () => clearInterval(iv);
    }, []);

    // ── Live events ─────────────────────────────────────────────────────────────

    useEffect(() => {
        const off1 = onEncoderStatusChanged((e) => {
            setRuntime((prev) => {
                const rt = prev.get(e.id);
                if (!rt) return prev;
                const next = new Map(prev);
                next.set(e.id, {
                    ...rt,
                    status: e.status,
                    error: e.error ?? null,
                    listeners: e.listeners !== undefined ? e.listeners : rt.listeners,
                });
                return next;
            });
        });
        const off2 = onListenerCountUpdated((e) => {
            setRuntime((prev) => {
                const rt = prev.get(e.encoderId);
                if (!rt) return prev;
                const next = new Map(prev);
                next.set(e.encoderId, { ...rt, listeners: e.count });
                return next;
            });
        });
        return () => {
            off1.then((fn) => fn());
            off2.then((fn) => fn());
        };
    }, []);

    // ── Derived ─────────────────────────────────────────────────────────────────

    const anyStreaming = [...runtime.values()].some((rt) => {
        const s = rt.status;
        if (typeof s === "string") return s === "streaming" || s === "recording";
        return false;
    });

    const totalListeners = [...runtime.values()].reduce(
        (sum, rt) => sum + (rt.listeners ?? 0),
        0
    );

    // ── Actions ─────────────────────────────────────────────────────────────────

    const handleStartAll = async () => {
        setStartingAll(true);
        try { await startAllEncoders(); } finally {
            setStartingAll(false);
            loadRuntime();
        }
    };

    const handleStopAll = async () => {
        setStoppingAll(true);
        try { await stopAllEncoders(); } finally {
            setStoppingAll(false);
            loadRuntime();
        }
    };

    const handleSaved = (_cfg: EncoderConfig) => {
        setEditTarget(null);
        loadEncoders();
        loadRuntime();
    };

    // ── Render ──────────────────────────────────────────────────────────────────

    const graphEncoders = encoders.filter((e) => e.output_type !== "file" && e.enabled);

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                height: "100%",
                overflow: "hidden",
                background: "var(--bg-surface)",
            }}
        >
            {/* Page header */}
            <div
                style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "10px 16px",
                    borderBottom: "1px solid var(--border-default)",
                    flexShrink: 0,
                    background: "var(--bg-panel)",
                }}
            >
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <Radio size={16} style={{ color: "var(--amber)" }} />
                    <span style={{ fontWeight: 700, fontSize: 14, letterSpacing: "0.04em" }}>
                        Streaming &amp; Encoders
                    </span>
                    {totalListeners > 0 && (
                        <span
                            style={{
                                background: "var(--cyan-glow)",
                                border: "1px solid var(--cyan-dim)",
                                color: "var(--cyan)",
                                borderRadius: "var(--r-full)",
                                fontSize: 10,
                                fontWeight: 700,
                                padding: "2px 8px",
                            }}
                        >
                            {totalListeners} LISTENERS
                        </span>
                    )}
                </div>

                <div style={{ display: "flex", gap: 6 }}>
                    <button
                        className="btn btn-ghost"
                        style={{ fontSize: 11, padding: "4px 8px" }}
                        onClick={loadRuntime}
                    >
                        <RefreshCw size={11} />
                    </button>
                    {anyStreaming ? (
                        <button
                            className="btn btn-danger"
                            style={{ fontSize: 11, padding: "4px 12px" }}
                            disabled={stoppingAll}
                            onClick={handleStopAll}
                        >
                            <Square size={11} /> Stop All
                        </button>
                    ) : (
                        <button
                            className="btn btn-primary"
                            style={{ fontSize: 11, padding: "4px 12px" }}
                            disabled={startingAll || encoders.filter((e) => e.enabled).length === 0}
                            onClick={handleStartAll}
                        >
                            <Play size={11} /> Start All
                        </button>
                    )}
                </div>
            </div>

            {/* Body: left panel (list) + right main area */}
            <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
                {/* Left: encoder list */}
                <div
                    style={{
                        width: 280,
                        flexShrink: 0,
                        borderRight: "1px solid var(--border-default)",
                        background: "var(--bg-panel)",
                        display: "flex",
                        flexDirection: "column",
                        overflow: "hidden",
                    }}
                >
                    <EncoderList
                        encoders={encoders}
                        runtime={runtime}
                        onEdit={(cfg) => setEditTarget(cfg)}
                        onNew={() => setEditTarget("new")}
                        onRefresh={() => { loadEncoders(); loadRuntime(); }}
                    />
                </div>

                {/* Right: status cards + graph */}
                <div
                    style={{
                        flex: 1,
                        display: "flex",
                        flexDirection: "column",
                        gap: 0,
                        overflow: "auto",
                        padding: 16,
                    }}
                >
                    {/* Status cards */}
                    <div style={{ marginBottom: 16 }}>
                        <div className="section-label" style={{ marginBottom: 8 }}>
                            Active Encoders
                        </div>
                        <EncoderStatusCards
                            configs={encoders}
                            runtime={runtime}
                            onEdit={(id) => {
                                const cfg = encoders.find((e) => e.id === id);
                                if (cfg) setEditTarget(cfg);
                            }}
                            onStart={async (id) => {
                                const { startEncoder } = await import("../lib/bridge");
                                await startEncoder(id);
                                loadRuntime();
                            }}
                            onStop={async (id) => {
                                const { stopEncoder } = await import("../lib/bridge");
                                await stopEncoder(id);
                                loadRuntime();
                            }}
                        />
                    </div>

                    {/* Listener graph — only shown if there are network encoders */}
                    {graphEncoders.length > 0 && (
                        <div>
                            <ListenerGraph
                                encoders={graphEncoders}
                                snapshots={snapshots}
                                activePeriod={period}
                                onPeriodChange={setPeriod}
                            />
                        </div>
                    )}
                </div>
            </div>

            {/* Editor dialog */}
            {editTarget !== null && (
                <EncoderEditor
                    initial={editTarget === "new" ? null : editTarget}
                    onClose={() => setEditTarget(null)}
                    onSaved={handleSaved}
                />
            )}
        </div>
    );
}
