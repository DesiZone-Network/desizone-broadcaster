import { useEffect, useRef, forwardRef } from "react";
import type { VuEvent } from "../../lib/bridge";

interface Props {
    vuData: VuEvent | null;
    height?: number;
    width?: number;
    orientation?: "vertical" | "horizontal";
    compact?: boolean;
}

export const VUMeter = forwardRef<HTMLCanvasElement, Props>(
    ({ vuData, height = 48, width = 32, orientation = "vertical", compact = false }, ref) => {
        const internalRef = useRef<HTMLCanvasElement>(null);
        const canvasRef = (ref as React.RefObject<HTMLCanvasElement>) ?? internalRef;

        useEffect(() => {
            const canvas = canvasRef.current;
            if (!canvas) return;
            const ctx = canvas.getContext("2d")!;
            const W = canvas.width;
            const H = canvas.height;
            ctx.clearRect(0, 0, W, H);

            const leftDb = vuData?.left_db ?? -60;
            const rightDb = vuData?.right_db ?? -60;

            // Background
            ctx.fillStyle = "#0f0f12";
            ctx.fillRect(0, 0, W, H);

            if (orientation === "vertical") {
                const barW = Math.floor((W - 3) / 2);

                const drawVBar = (x: number, bw: number, db: number) => {
                    const pct = Math.max(0, Math.min(1, (db + 60) / 60));
                    const fillH = Math.round(pct * (H - 4));
                    const y = H - 2 - fillH;

                    // Segments
                    const segH = 3;
                    const segGap = 1;
                    const numSegs = Math.floor((H - 4) / (segH + segGap));

                    for (let i = 0; i < numSegs; i++) {
                        const sy = H - 2 - (i + 1) * (segH + segGap) + segGap;
                        const frac = i / numSegs;
                        const isLit = sy >= y;

                        if (frac > 0.85) {
                            ctx.fillStyle = isLit ? "#dc2626" : "#1a0a0a";
                        } else if (frac > 0.65) {
                            ctx.fillStyle = isLit ? "#ca8a04" : "#1a1400";
                        } else {
                            ctx.fillStyle = isLit ? "#16a34a" : "#0a1a0a";
                        }
                        ctx.fillRect(x + 1, sy, bw - 2, segH);
                    }
                };

                drawVBar(0, barW, leftDb);
                drawVBar(barW + 3, barW, rightDb);
            } else {
                // Horizontal
                const barH = Math.floor((H - 3) / 2);
                const drawHBar = (y: number, bh: number, db: number) => {
                    const pct = Math.max(0, Math.min(1, (db + 60) / 60));
                    const fillW = Math.round(pct * (W - 4));

                    const numSegs = Math.floor((W - 4) / 4);
                    for (let i = 0; i < numSegs; i++) {
                        const sx = 2 + i * 4;
                        const frac = i / numSegs;
                        const isLit = sx < 2 + fillW;
                        if (frac > 0.85) {
                            ctx.fillStyle = isLit ? "#dc2626" : "#1a0a0a";
                        } else if (frac > 0.65) {
                            ctx.fillStyle = isLit ? "#ca8a04" : "#1a1400";
                        } else {
                            ctx.fillStyle = isLit ? "#16a34a" : "#0a1a0a";
                        }
                        ctx.fillRect(sx, y + 1, 3, bh - 2);
                    }
                };
                drawHBar(0, barH, leftDb);
                drawHBar(barH + 3, barH, rightDb);
            }
        }, [vuData, canvasRef, orientation]);

        return (
            <div className="flex flex-col items-center gap-1">
                <canvas
                    ref={canvasRef}
                    width={width}
                    height={height}
                    style={{ borderRadius: 3, border: "1px solid var(--border-subtle)" }}
                />
                {!compact && (
                    <div className="flex gap-2">
                        <span className="text-xs mono" style={{ fontSize: 9, color: "var(--text-muted)" }}>L</span>
                        <span className="text-xs mono" style={{ fontSize: 9, color: "var(--text-muted)" }}>R</span>
                    </div>
                )}
            </div>
        );
    }
);

VUMeter.displayName = "VUMeter";
