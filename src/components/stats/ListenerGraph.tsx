import { useEffect, useRef, useState, useCallback } from "react";
import {
    ListenerSnapshot,
    EncoderConfig,
    StatsPeriod,
    onListenerCountUpdated,
} from "../../lib/bridge";


interface TooltipState {
    x: number;
    y: number;
    content: string;
    visible: boolean;
}

// Colour palette for multi-encoder lines
const LINE_COLORS = [
    "#06b6d4", // cyan
    "#f59e0b", // amber
    "#22c55e", // green
    "#8b5cf6", // purple
    "#ef4444", // red
    "#fb923c", // orange
];

interface Props {
    encoders: EncoderConfig[];
    /** Pre-loaded snapshots keyed by encoder id */
    snapshots: Map<number, ListenerSnapshot[]>;
    onPeriodChange: (period: StatsPeriod) => void;
    activePeriod: StatsPeriod;
}

export function ListenerGraph({
    encoders,
    snapshots,
    onPeriodChange,
    activePeriod,
}: Props) {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const wrapRef = useRef<HTMLDivElement>(null);
    const [tooltip, setTooltip] = useState<TooltipState>({
        x: 0,
        y: 0,
        content: "",
        visible: false,
    });
    // live data buffer: map encoder id → last N counts
    const [liveData, setLiveData] = useState<Map<number, number[]>>(new Map());

    // Subscribe to live listener events
    useEffect(() => {
        const unsub = onListenerCountUpdated((e) => {
            setLiveData((prev) => {
                const arr = [...(prev.get(e.encoderId) ?? []), e.count].slice(-120);
                return new Map(prev).set(e.encoderId, arr);
            });
        });
        return () => { unsub.then((fn) => fn()); };
    }, []);

    // Build merged dataset: combine historical snapshots + live deltas
    const buildDataset = useCallback((): { enc: EncoderConfig; points: { ts: number; count: number }[] }[] => {
        return encoders.map((enc) => {
            const historical = (snapshots.get(enc.id) ?? []).map((s) => ({
                ts: s.snapshot_at * 1000,
                count: s.current_listeners,
            }));
            return { enc, points: historical };
        });
    }, [encoders, snapshots]);

    // Canvas drawing
    useEffect(() => {
        const canvas = canvasRef.current;
        const wrap = wrapRef.current;
        if (!canvas || !wrap) return;

        const dpr = window.devicePixelRatio || 1;
        const rect = wrap.getBoundingClientRect();
        const W = rect.width - 16; // 8px padding each side
        const H = 160;

        canvas.width = W * dpr;
        canvas.height = H * dpr;
        canvas.style.width = `${W}px`;
        canvas.style.height = `${H}px`;

        const ctx = canvas.getContext("2d")!;
        ctx.scale(dpr, dpr);
        ctx.clearRect(0, 0, W, H);

        const dataset = buildDataset().filter((d) => d.points.length > 1);

        if (dataset.length === 0) {
            ctx.fillStyle = "rgba(255,255,255,0.15)";
            ctx.font = "12px Inter, sans-serif";
            ctx.textAlign = "center";
            ctx.fillText("No listener data available", W / 2, H / 2);
            return;
        }

        // Determine global min/max time and max count
        const allTs = dataset.flatMap((d) => d.points.map((p) => p.ts));
        const allCounts = dataset.flatMap((d) => d.points.map((p) => p.count));
        const minTs = Math.min(...allTs);
        const maxTs = Math.max(...allTs);
        const maxCount = Math.max(...allCounts, 1);

        const PAD_L = 40;
        const PAD_R = 12;
        const PAD_T = 12;
        const PAD_B = 24;
        const chartW = W - PAD_L - PAD_R;
        const chartH = H - PAD_T - PAD_B;

        const toX = (ts: number) =>
            PAD_L + ((ts - minTs) / Math.max(maxTs - minTs, 1)) * chartW;
        const toY = (count: number) =>
            PAD_T + (1 - count / maxCount) * chartH;

        // Grid lines
        ctx.strokeStyle = "#25253240";
        ctx.lineWidth = 1;
        const gridLines = 4;
        for (let i = 0; i <= gridLines; i++) {
            const y = PAD_T + (chartH / gridLines) * i;
            ctx.beginPath();
            ctx.moveTo(PAD_L, y);
            ctx.lineTo(PAD_L + chartW, y);
            ctx.stroke();
            // Y label
            const label = Math.round(maxCount * (1 - i / gridLines));
            ctx.fillStyle = "#60607a";
            ctx.font = "9px JetBrains Mono, monospace";
            ctx.textAlign = "right";
            ctx.fillText(String(label), PAD_L - 4, y + 3);
        }

        // X axis time labels
        ctx.fillStyle = "#60607a";
        ctx.font = "9px JetBrains Mono, monospace";
        ctx.textAlign = "center";
        const xLabels = 4;
        for (let i = 0; i <= xLabels; i++) {
            const ts = minTs + ((maxTs - minTs) / xLabels) * i;
            const x = toX(ts);
            const d = new Date(ts);
            const label = `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
            ctx.fillText(label, x, H - 6);
        }

        // Draw lines per encoder
        dataset.forEach(({ enc, points }, idx) => {
            const color = LINE_COLORS[idx % LINE_COLORS.length];

            ctx.save();
            ctx.beginPath();
            points.forEach(({ ts, count }, i) => {
                const x = toX(ts);
                const y = toY(count);
                if (i === 0) ctx.moveTo(x, y);
                else ctx.lineTo(x, y);
            });
            ctx.strokeStyle = color;
            ctx.lineWidth = 2;
            ctx.lineJoin = "round";
            ctx.stroke();

            // Fill area
            ctx.lineTo(toX(points[points.length - 1].ts), PAD_T + chartH);
            ctx.lineTo(toX(points[0].ts), PAD_T + chartH);
            ctx.closePath();
            const grad = ctx.createLinearGradient(0, PAD_T, 0, PAD_T + chartH);
            grad.addColorStop(0, color + "30");
            grad.addColorStop(1, color + "00");
            ctx.fillStyle = grad;
            ctx.fill();
            ctx.restore();

            // Encoder label
            if (points.length > 0) {
                const last = points[points.length - 1];
                ctx.fillStyle = color;
                ctx.font = "9px Inter, sans-serif";
                ctx.textAlign = "left";
                ctx.fillText(enc.name, toX(last.ts) + 4, toY(last.count));
            }
        });
    }, [buildDataset, liveData]);

    // Hover tooltip
    const handleMouseMove = useCallback(
        (e: React.MouseEvent<HTMLCanvasElement>) => {
            const canvas = canvasRef.current;
            if (!canvas) return;
            const rect = canvas.getBoundingClientRect();
            const mx = e.clientX - rect.left;
            const my = e.clientY - rect.top;

            // Find closest data point across all encoders
            const dataset = buildDataset().filter((d) => d.points.length > 0);
            if (dataset.length === 0) return;

            const allTs = dataset.flatMap((d) => d.points.map((p) => p.ts));
            const minTs = Math.min(...allTs);
            const maxTs = Math.max(...allTs);
            const chartW = rect.width - 52;
            const tsAtX = minTs + ((mx - 40) / chartW) * (maxTs - minTs);

            const lines: string[] = [];
            dataset.forEach(({ enc, points }) => {
                const closest = points.reduce((a, b) =>
                    Math.abs(b.ts - tsAtX) < Math.abs(a.ts - tsAtX) ? b : a
                );
                const d = new Date(closest.ts);
                const label = `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
                lines.push(`${enc.name}: ${closest.count} listeners @ ${label}`);
            });

            setTooltip({
                x: mx + 10,
                y: my - 10,
                content: lines.join("\n"),
                visible: true,
            });
        },
        [buildDataset]
    );

    const handleMouseLeave = () =>
        setTooltip((t) => ({ ...t, visible: false }));

    const PERIODS: StatsPeriod[] = ["1h", "6h", "24h", "7d"];

    return (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {/* Header row */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span className="section-label">Listener Trend</span>
                    {/* Legend dots */}
                    {encoders.slice(0, 4).map((enc, i) => (
                        <span key={enc.id} style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 10, color: "var(--text-secondary)" }}>
                            <span style={{ width: 8, height: 8, borderRadius: "50%", background: LINE_COLORS[i] }} />
                            {enc.name}
                        </span>
                    ))}
                </div>
                <div className="listener-graph-period-btns">
                    {PERIODS.map((p) => (
                        <button
                            key={p}
                            className={`period-btn ${p === activePeriod ? "active" : ""}`}
                            onClick={() => onPeriodChange(p)}
                        >
                            {p}
                        </button>
                    ))}
                </div>
            </div>

            {/* Canvas */}
            <div className="listener-graph-wrap" ref={wrapRef}>
                <canvas
                    ref={canvasRef}
                    className="listener-graph-canvas"
                    onMouseMove={handleMouseMove}
                    onMouseLeave={handleMouseLeave}
                />
                {tooltip.visible && (
                    <div
                        className="listener-graph-tooltip"
                        style={{ left: tooltip.x, top: tooltip.y, whiteSpace: "pre" }}
                    >
                        {tooltip.content}
                    </div>
                )}
            </div>
        </div>
    );
}
