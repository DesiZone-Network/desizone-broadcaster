import { useEffect, useRef } from "react";

interface Props {
    waveformData: Float32Array | null;
    positionMs: number;
    durationMs: number;
    onSeek?: (positionMs: number) => void;
    width?: number;
    height?: number;
    color?: string;
    playheadColor?: string;
}

export function WaveformCanvas({
    waveformData,
    positionMs,
    durationMs,
    onSeek,
    width = 400,
    height = 64,
    color = "#f59e0b",
    playheadColor = "#ffffff",
}: Props) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d")!;
        const W = canvas.width;
        const H = canvas.height;
        ctx.clearRect(0, 0, W, H);

        // Background
        ctx.fillStyle = "#0f0f12";
        ctx.fillRect(0, 0, W, H);

        // Center line
        ctx.strokeStyle = "#252532";
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(0, H / 2);
        ctx.lineTo(W, H / 2);
        ctx.stroke();

        if (waveformData && waveformData.length > 0) {
            const playheadX = durationMs > 0 ? (positionMs / durationMs) * W : 0;

            // Waveform bars
            const barW = Math.max(1, W / waveformData.length);

            for (let i = 0; i < waveformData.length; i++) {
                const x = i * barW;
                const amp = Math.abs(waveformData[i]);
                const barH = amp * (H / 2 - 2);

                const isPast = x <= playheadX;

                // Top half
                ctx.fillStyle = isPast
                    ? color
                    : `${color}50`;
                ctx.fillRect(x, H / 2 - barH, barW - 0.5, barH);

                // Bottom (mirror, dimmer)
                ctx.fillStyle = isPast
                    ? `${color}70`
                    : `${color}25`;
                ctx.fillRect(x, H / 2, barW - 0.5, barH);
            }
        } else {
            // No data — draw placeholder static noise
            ctx.fillStyle = "#1a1a20";
            for (let x = 0; x < W; x += 3) {
                const h = (Math.sin(x * 0.15) * 0.3 + 0.3) * H * 0.4;
                ctx.fillRect(x, H / 2 - h / 2, 2, h);
            }
        }

        // Playhead
        if (durationMs > 0) {
            const px = (positionMs / durationMs) * W;
            ctx.strokeStyle = playheadColor;
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(px, 0);
            ctx.lineTo(px, H);
            ctx.stroke();

            // Playhead triangle
            ctx.fillStyle = playheadColor;
            ctx.beginPath();
            ctx.moveTo(px - 4, 0);
            ctx.lineTo(px + 4, 0);
            ctx.lineTo(px, 6);
            ctx.closePath();
            ctx.fill();
        }

        // Time markers
        if (durationMs > 0) {
            const numMarkers = 4;
            ctx.strokeStyle = "#333342";
            ctx.lineWidth = 1;
            for (let i = 1; i < numMarkers; i++) {
                const x = (i / numMarkers) * W;
                ctx.beginPath();
                ctx.moveTo(x, H - 8);
                ctx.lineTo(x, H);
                ctx.stroke();
            }
        }
    }, [waveformData, positionMs, durationMs, color, playheadColor]);

    const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
        if (!onSeek || durationMs === 0) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const ratio = (e.clientX - rect.left) / rect.width;
        onSeek(Math.round(ratio * durationMs));
    };

    return (
        <div className="waveform-wrap" style={{ height }}>
            <canvas
                ref={canvasRef}
                width={width}
                height={height}
                className="waveform-canvas"
                style={{ height: "100%", cursor: onSeek ? "crosshair" : "default" }}
                onClick={handleClick}
            />
        </div>
    );
}
