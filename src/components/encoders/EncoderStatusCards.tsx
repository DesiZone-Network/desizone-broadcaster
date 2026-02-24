import { useEffect, useRef, useState } from "react";
import {
    EncoderConfig,
    EncoderRuntimeState,
    EncoderStatus,
    getListenerStats,
    onListenerCountUpdated,
} from "../../lib/bridge";

// ── Helper: status label + CSS class ─────────────────────────────────────────

function getStatusCls(status: EncoderStatus): string {
    if (typeof status === "string") return `enc-badge-${status}`;
    if ("retrying" in status) return "enc-badge-retrying";
    return "enc-badge-disabled";
}

function getStatusLabel(status: EncoderStatus): string {
    if (typeof status === "string") return status.toUpperCase();
    if ("retrying" in status) {
        const { attempt, max } = status.retrying;
        return `RETRYING ${attempt}${max > 0 ? `/${max}` : ""}`;
    }
    return "UNKNOWN";
}

function isActiveStatus(status: EncoderStatus): boolean {
    if (typeof status === "string")
        return status === "streaming" || status === "recording" || status === "connecting";
    return "retrying" in status;
}

// ── Status dot ────────────────────────────────────────────────────────────────

function StatusDot({ status }: { status: EncoderStatus }) {
    const active = isActiveStatus(status);
    let color = "var(--text-muted)";
    if (typeof status === "string") {
        if (status === "streaming") color = "var(--cyan)";
        else if (status === "recording") color = "var(--red)";
        else if (status === "connecting") color = "var(--amber)";
        else if (status === "failed") color = "var(--red)";
    } else if ("retrying" in status) {
        color = "var(--amber)";
    }

    return (
        <span
            style={{
                display: "inline-block",
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: color,
                flexShrink: 0,
                animation: active ? "pulse-dot 1.2s ease-in-out infinite" : "none",
            }}
        />
    );
}

// ── Mini listener sparkline ───────────────────────────────────────────────────

function Sparkline({ data }: { data: number[] }) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas || data.length < 2) return;
        const ctx = canvas.getContext("2d")!;
        const W = canvas.width;
        const H = canvas.height;
        ctx.clearRect(0, 0, W, H);

        const max = Math.max(...data, 1);
        const min = 0;

        ctx.beginPath();
        data.forEach((v, i) => {
            const x = (i / (data.length - 1)) * W;
            const y = H - ((v - min) / (max - min)) * (H - 4) - 2;
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        });

        ctx.strokeStyle = "var(--cyan)";
        ctx.lineWidth = 1.5;
        ctx.stroke();

        // Fill area under line
        ctx.lineTo(W, H);
        ctx.lineTo(0, H);
        ctx.closePath();
        const grad = ctx.createLinearGradient(0, 0, 0, H);
        grad.addColorStop(0, "#06b6d430");
        grad.addColorStop(1, "#06b6d400");
        ctx.fillStyle = grad;
        ctx.fill();
    }, [data]);

    return (
        <canvas
            ref={canvasRef}
            width={80}
            height={28}
            style={{ borderRadius: "var(--r-sm)" }}
        />
    );
}

// ── EncoderStatusCards ────────────────────────────────────────────────────────

interface Props {
    configs: EncoderConfig[];
    runtime: Map<number, EncoderRuntimeState>;
    onEdit: (id: number) => void;
    onStart: (id: number) => void;
    onStop: (id: number) => void;
}

export function EncoderStatusCards({
    configs,
    runtime,
    onEdit,
    onStart,
    onStop,
}: Props) {
    // Per-encoder sparkline data (last ~20 readings from event bus)
    const [sparklines, setSparklines] = useState<Map<number, number[]>>(
        new Map()
    );

    // Fetch initial 1h history for each streaming encoder
    useEffect(() => {
        configs.forEach(async (cfg) => {
            const rt = runtime.get(cfg.id);
            if (!rt || typeof rt.status !== "string" || rt.status !== "streaming")
                return;
            try {
                const snaps = await getListenerStats(cfg.id, "1h");
                setSparklines((prev) =>
                    new Map(prev).set(
                        cfg.id,
                        snaps.map((s) => s.current_listeners)
                    )
                );
            } catch { /* no DB yet */ }
        });
    }, [configs, runtime]);

    // Live listener count updates
    useEffect(() => {
        const unsub = onListenerCountUpdated((e) => {
            setSparklines((prev) => {
                const arr = [...(prev.get(e.encoderId) ?? []), e.count].slice(-40);
                return new Map(prev).set(e.encoderId, arr);
            });
        });
        return () => { unsub.then((fn) => fn()); };
    }, []);

    if (configs.length === 0) {
        return (
            <div
                style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    height: 80,
                    color: "var(--text-muted)",
                    fontSize: 12,
                }}
            >
                No encoders configured
            </div>
        );
    }

    return (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {configs.map((cfg) => {
                const rt = runtime.get(cfg.id);
                const status: EncoderStatus = rt?.status ?? "disabled";
                const cls = getStatusCls(status);
                const listeners = rt?.listeners ?? null;
                const bytesSent = rt?.bytes_sent ?? 0;
                const spark = sparklines.get(cfg.id) ?? [];
                const isRunning = isActiveStatus(status);
                const error = rt?.error;

                const cardCls = ["encoder-card"];
                if (typeof status === "string") {
                    if (status === "streaming") cardCls.push("streaming");
                    if (status === "recording") cardCls.push("recording");
                    if (status === "failed") cardCls.push("failed");
                }

                const outputLabel =
                    cfg.output_type === "file"
                        ? `File (${cfg.file_rotation})`
                        : `${cfg.output_type.toUpperCase()} — ${cfg.server_host ?? ""}:${cfg.server_port ?? ""}${cfg.mount_point ?? ""}`;

                const codecLabel = cfg.output_type === "file"
                    ? cfg.codec.toUpperCase()
                    : `${cfg.codec.toUpperCase()} ${cfg.bitrate_kbps ?? ""}kbps`;

                return (
                    <div key={cfg.id} className={cardCls.join(" ")}>
                        {/* Header */}
                        <div className="encoder-card-header">
                            <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
                                <StatusDot status={status} />
                                <span className="encoder-card-name">{cfg.name}</span>
                                <span className={`enc-badge ${cls}`}>
                                    {getStatusLabel(status)}
                                </span>
                            </div>
                            <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                                <button className="btn btn-ghost" style={{ padding: "3px 8px", fontSize: 11 }} onClick={() => onEdit(cfg.id)}>
                                    ✎ Edit
                                </button>
                                {isRunning ? (
                                    <button className="btn btn-danger" style={{ padding: "3px 8px", fontSize: 11 }} onClick={() => onStop(cfg.id)}>
                                        ■ Stop
                                    </button>
                                ) : (
                                    <button className="btn btn-primary" style={{ padding: "3px 8px", fontSize: 11 }} disabled={!cfg.enabled} onClick={() => onStart(cfg.id)}>
                                        ▶ Start
                                    </button>
                                )}
                            </div>
                        </div>

                        {/* Sub-info row */}
                        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                            <span className="encoder-card-meta">{outputLabel}</span>
                            <span className="encoder-card-meta">•</span>
                            <span className="encoder-card-meta">{codecLabel}</span>
                        </div>

                        {/* Error row */}
                        {error && (
                            <div style={{ fontSize: 10, color: "var(--red)", fontFamily: "var(--font-mono)", padding: "2px 0" }}>
                                ✕ {error}
                            </div>
                        )}

                        {/* Stats + sparkline */}
                        {rt && isRunning && (
                            <div className="encoder-card-stats">
                                {listeners !== null && cfg.output_type !== "file" && (
                                    <div className="encoder-stat">
                                        <span className="encoder-stat-label">Listeners</span>
                                        <span className={`encoder-stat-value ${listeners > 0 ? "cyan" : ""}`}>
                                            {listeners}
                                        </span>
                                    </div>
                                )}
                                {cfg.output_type === "file" && rt.recording_file && (
                                    <div className="encoder-stat" style={{ flex: 1, minWidth: 0 }}>
                                        <span className="encoder-stat-label">Recording</span>
                                        <span className="encoder-stat-value" style={{ fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                            {rt.recording_file.split("/").pop()}
                                        </span>
                                    </div>
                                )}
                                <div className="encoder-stat">
                                    <span className="encoder-stat-label">Data sent</span>
                                    <span className="encoder-stat-value">{formatBytes(bytesSent)}</span>
                                </div>
                                {rt.uptime_secs > 0 && (
                                    <div className="encoder-stat">
                                        <span className="encoder-stat-label">Uptime</span>
                                        <span className="encoder-stat-value green">{formatDuration(rt.uptime_secs)}</span>
                                    </div>
                                )}
                                {spark.length > 2 && cfg.output_type !== "file" && (
                                    <div className="encoder-stat" style={{ marginLeft: "auto" }}>
                                        <span className="encoder-stat-label">Trend (1h)</span>
                                        <Sparkline data={spark} />
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                );
            })}
        </div>
    );
}

// ── Formatters ────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function formatDuration(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s}s`;
    return `${s}s`;
}
