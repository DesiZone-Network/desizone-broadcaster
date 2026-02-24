import { useEffect, useRef } from "react";
import type { FadeCurve } from "../../lib/bridge";

interface Props {
    outCurve: FadeCurve;
    inCurve: FadeCurve;
    outTimeMs: number;
    inTimeMs: number;
    crossfadePointMs?: number;
    progress?: number; // 0-1 for live animation
    width?: number;
    height?: number;
}

function evalCurve(curve: FadeCurve, t: number): number {
    switch (curve) {
        case "linear": return 1 - t;
        case "exponential": return (1 - t) * (1 - t);
        case "s_curve": return 0.5 * (1 + Math.cos(Math.PI * t));
        case "logarithmic": return Math.log10(1 + 9 * (1 - t)) / Math.log10(10);
        case "constant_power": return Math.cos(t * Math.PI / 2);
        default: return 1 - t;
    }
}

function evalCurveIn(curve: FadeCurve, t: number): number {
    if (curve === "constant_power") return Math.sin(t * Math.PI / 2);
    return evalCurve(curve, 1 - t);
}

export function FadeCurveGraph({
    outCurve, inCurve,
    outTimeMs, inTimeMs,
    crossfadePointMs,
    progress,
    width = 440,
    height = 140,
}: Props) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d")!;
        const W = canvas.width;
        const H = canvas.height;
        const pad = { top: 12, right: 16, bottom: 20, left: 36 };
        const gW = W - pad.left - pad.right;
        const gH = H - pad.top - pad.bottom;

        ctx.clearRect(0, 0, W, H);
        ctx.fillStyle = "#0a0a0e";
        ctx.fillRect(0, 0, W, H);

        // Grid
        ctx.strokeStyle = "#1e1e28";
        ctx.lineWidth = 1;
        for (let i = 0; i <= 4; i++) {
            const y = pad.top + (i / 4) * gH;
            ctx.beginPath();
            ctx.moveTo(pad.left, y);
            ctx.lineTo(W - pad.right, y);
            ctx.stroke();
        }
        for (let i = 0; i <= 4; i++) {
            const x = pad.left + (i / 4) * gW;
            ctx.beginPath();
            ctx.moveTo(x, pad.top);
            ctx.lineTo(x, pad.top + gH);
            ctx.stroke();
        }

        // DB axis labels
        ctx.fillStyle = "#404050";
        ctx.font = "9px JetBrains Mono, monospace";
        ctx.textAlign = "right";
        const labels = ["0", "−6", "−12", "−18", "−∞"];
        labels.forEach((l, i) => {
            ctx.fillText(l, pad.left - 4, pad.top + (i / 4) * gH + 3);
        });

        const steps = 200;

        // Fade-out curve (amber/orange)
        ctx.beginPath();
        ctx.strokeStyle = "#f59e0b";
        ctx.lineWidth = 2;
        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const gain = evalCurve(outCurve, t);
            const x = pad.left + t * gW;
            const y = pad.top + (1 - gain) * gH;
            if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        }
        ctx.stroke();

        // Fade-out fill
        ctx.beginPath();
        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const gain = evalCurve(outCurve, t);
            const x = pad.left + t * gW;
            const y = pad.top + (1 - gain) * gH;
            if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        }
        ctx.lineTo(pad.left + gW, pad.top + gH);
        ctx.lineTo(pad.left, pad.top + gH);
        ctx.closePath();
        ctx.fillStyle = "#f59e0b14";
        ctx.fill();

        // Fade-in curve (cyan)
        ctx.beginPath();
        ctx.strokeStyle = "#06b6d4";
        ctx.lineWidth = 2;
        ctx.setLineDash([]);
        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const gain = evalCurveIn(inCurve, t);
            const x = pad.left + t * gW;
            const y = pad.top + (1 - gain) * gH;
            if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        }
        ctx.stroke();

        // Fade-in fill
        ctx.beginPath();
        for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            const gain = evalCurveIn(inCurve, t);
            const x = pad.left + t * gW;
            const y = pad.top + (1 - gain) * gH;
            if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        }
        ctx.lineTo(pad.left + gW, pad.top + gH);
        ctx.lineTo(pad.left, pad.top + gH);
        ctx.closePath();
        ctx.fillStyle = "#06b6d414";
        ctx.fill();

        // Crossfade point (dashed red)
        if (crossfadePointMs != null && outTimeMs > 0) {
            const xfPct = Math.min(1, crossfadePointMs / outTimeMs);
            const xfX = pad.left + xfPct * gW;
            ctx.strokeStyle = "#ef444480";
            ctx.lineWidth = 1;
            ctx.setLineDash([3, 3]);
            ctx.beginPath();
            ctx.moveTo(xfX, pad.top);
            ctx.lineTo(xfX, pad.top + gH);
            ctx.stroke();
            ctx.setLineDash([]);

            ctx.fillStyle = "#ef4444";
            ctx.font = "9px Inter, sans-serif";
            ctx.textAlign = "center";
            ctx.fillText("XF", xfX, pad.top + gH + 12);
        }

        // Progress needle
        if (progress != null && progress >= 0 && progress <= 1) {
            const px = pad.left + progress * gW;
            ctx.strokeStyle = "#ffffff60";
            ctx.lineWidth = 1.5;
            ctx.setLineDash([2, 2]);
            ctx.beginPath();
            ctx.moveTo(px, pad.top);
            ctx.lineTo(px, pad.top + gH);
            ctx.stroke();
            ctx.setLineDash([]);
        }

        // Legend
        ctx.font = "10px Inter, sans-serif";
        ctx.textAlign = "left";
        ctx.fillStyle = "#f59e0b";
        ctx.fillRect(pad.left, 2, 12, 2);
        ctx.fillText("Fade Out", pad.left + 16, 8);
        ctx.fillStyle = "#06b6d4";
        ctx.fillRect(pad.left + 80, 2, 12, 2);
        ctx.fillText("Fade In", pad.left + 96, 8);
    }, [outCurve, inCurve, outTimeMs, inTimeMs, crossfadePointMs, progress]);

    return (
        <canvas
            ref={canvasRef}
            width={width}
            height={height}
            style={{
                display: "block",
                borderRadius: "var(--r-md)",
                border: "1px solid var(--border-default)",
                width: "100%",
            }}
        />
    );
}
