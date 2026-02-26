import { useEffect, useRef, useState } from "react";

interface Props {
    waveformData: Float32Array | null;
    positionMs: number;
    durationMs: number;
    isPlaying?: boolean;
    playbackRate?: number;
    animatePlayhead?: boolean;
    scrollWithPlayhead?: boolean;
    scrollWindowMs?: number;
    onSeek?: (positionMs: number) => void;
    cueMarkers?: Array<{ positionMs: number; color: string; label?: string }>;
    beatGridMs?: number[] | null;
    loopRange?: { startMs: number; endMs: number; beats?: number } | null;
    onAltSetCue?: (positionMs: number) => void;
    width?: number;
    height?: number;
    color?: string;
    playheadColor?: string;
}

export function WaveformCanvas({
    waveformData,
    positionMs,
    durationMs,
    isPlaying = false,
    playbackRate = 1,
    animatePlayhead = true,
    scrollWithPlayhead = false,
    scrollWindowMs = 12000,
    onSeek,
    cueMarkers = [],
    beatGridMs = null,
    loopRange = null,
    onAltSetCue,
    width = 0,
    height = 64,
    color = "#f59e0b",
    playheadColor = "#ffffff",
}: Props) {
    const wrapRef = useRef<HTMLDivElement>(null);
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const [canvasWidth, setCanvasWidth] = useState(width);
    const [animatedPosMs, setAnimatedPosMs] = useState(positionMs);

    const targetPosRef = useRef(positionMs);
    const animatedPosRef = useRef(positionMs);
    const rafRef = useRef<number | null>(null);
    const lastTsRef = useRef<number | null>(null);
    const viewStartRef = useRef(0);
    const viewDurationRef = useRef(1);

    useEffect(() => {
        const wrap = wrapRef.current;
        if (!wrap) return;
        if (width > 0) {
            setCanvasWidth(width);
            return;
        }
        const update = () => {
            const w = Math.max(1, Math.floor(wrap.clientWidth));
            setCanvasWidth(w);
        };
        update();
        const ro = new ResizeObserver(update);
        ro.observe(wrap);
        return () => ro.disconnect();
    }, [width, height]);

    useEffect(() => {
        targetPosRef.current = positionMs;
        const jumpMs = Math.abs(positionMs - animatedPosRef.current);
        if (!isPlaying || jumpMs > 250) {
            animatedPosRef.current = positionMs;
            setAnimatedPosMs(positionMs);
        }
    }, [positionMs, isPlaying]);

    useEffect(() => {
        if (!animatePlayhead || !isPlaying) {
            if (rafRef.current != null) {
                cancelAnimationFrame(rafRef.current);
                rafRef.current = null;
            }
            lastTsRef.current = null;
            return;
        }

        const step = (ts: number) => {
            const last = lastTsRef.current ?? ts;
            const dtMs = Math.min(120, Math.max(0, ts - last));
            lastTsRef.current = ts;

            const rate = Number.isFinite(playbackRate) ? playbackRate : 1;
            let next = animatedPosRef.current + dtMs * rate;
            const target = targetPosRef.current;
            const drift = target - next;

            if (Math.abs(drift) > 300) {
                next = target;
            } else {
                next += drift * 0.18;
            }

            if (durationMs > 0) {
                next = Math.min(durationMs, Math.max(0, next));
            } else {
                next = Math.max(0, next);
            }

            if (Math.abs(next - animatedPosRef.current) >= 0.1) {
                animatedPosRef.current = next;
                setAnimatedPosMs(next);
            }
            rafRef.current = requestAnimationFrame(step);
        };

        rafRef.current = requestAnimationFrame(step);
        return () => {
            if (rafRef.current != null) {
                cancelAnimationFrame(rafRef.current);
                rafRef.current = null;
            }
            lastTsRef.current = null;
        };
    }, [animatePlayhead, isPlaying, playbackRate, durationMs]);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;

        const dpr = window.devicePixelRatio || 1;
        const cssW = Math.max(1, canvasWidth);
        const cssH = Math.max(1, height);
        const pixelW = Math.max(1, Math.floor(cssW * dpr));
        const pixelH = Math.max(1, Math.floor(cssH * dpr));

        if (canvas.width !== pixelW) canvas.width = pixelW;
        if (canvas.height !== pixelH) canvas.height = pixelH;

        const ctx = canvas.getContext("2d");
        if (!ctx) return;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

        const W = cssW;
        const H = cssH;

        ctx.clearRect(0, 0, W, H);
        ctx.fillStyle = "#0f0f12";
        ctx.fillRect(0, 0, W, H);

        ctx.strokeStyle = "#252532";
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(0, H / 2);
        ctx.lineTo(W, H / 2);
        ctx.stroke();

        const rawPosMs = animatePlayhead ? animatedPosMs : positionMs;
        const playPosMs = durationMs > 0
            ? Math.max(0, Math.min(durationMs, rawPosMs))
            : Math.max(0, rawPosMs);

        const isScrolling = scrollWithPlayhead && durationMs > 0;
        let viewStartMs = 0;
        let viewEndMs = durationMs;
        if (isScrolling) {
            const windowMs = Math.max(1000, Math.min(scrollWindowMs, durationMs));
            const half = windowMs / 2;
            viewStartMs = playPosMs - half;
            if (viewStartMs < 0) viewStartMs = 0;
            const maxStart = Math.max(0, durationMs - windowMs);
            if (viewStartMs > maxStart) viewStartMs = maxStart;
            viewEndMs = viewStartMs + windowMs;
        }

        const viewDurationMs = Math.max(1, viewEndMs - viewStartMs);
        viewStartRef.current = viewStartMs;
        viewDurationRef.current = viewDurationMs;

        const msToX = (ms: number) => ((ms - viewStartMs) / viewDurationMs) * W;

        if (durationMs > 0 && beatGridMs && beatGridMs.length > 0) {
            for (let i = 0; i < beatGridMs.length; i++) {
                const beatMs = beatGridMs[i];
                if (isScrolling && (beatMs < viewStartMs || beatMs > viewEndMs)) continue;
                const x = msToX(beatMs);
                if (x < 0 || x > W) continue;
                const isBar = i % 4 === 0;
                ctx.strokeStyle = isBar ? "#2f4f7a" : "#233141";
                ctx.lineWidth = isBar ? 1.2 : 1;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, H);
                ctx.stroke();
            }
        }

        if (waveformData && waveformData.length > 0) {
            if (isScrolling && durationMs > 0) {
                const totalBins = waveformData.length;
                const startBin = Math.max(0, Math.floor((viewStartMs / durationMs) * totalBins));
                const endBin = Math.min(totalBins, Math.ceil((viewEndMs / durationMs) * totalBins));
                const visibleBins = Math.max(1, endBin - startBin);
                const barW = Math.max(0.75, W / visibleBins);

                for (let i = startBin; i < endBin; i++) {
                    const x = (i - startBin) * barW;
                    const amp = Math.abs(waveformData[i]);
                    const barH = amp * (H / 2 - 2);
                    const binMs = ((i + 0.5) / totalBins) * durationMs;
                    const isPast = binMs <= playPosMs;

                    ctx.fillStyle = isPast ? color : `${color}50`;
                    ctx.fillRect(x, H / 2 - barH, Math.max(0.5, barW - 0.25), barH);

                    ctx.fillStyle = isPast ? `${color}70` : `${color}25`;
                    ctx.fillRect(x, H / 2, Math.max(0.5, barW - 0.25), barH);
                }
            } else {
                const playheadX = durationMs > 0 ? (playPosMs / durationMs) * W : 0;
                const barW = Math.max(0.75, W / waveformData.length);
                for (let i = 0; i < waveformData.length; i++) {
                    const x = i * barW;
                    const amp = Math.abs(waveformData[i]);
                    const barH = amp * (H / 2 - 2);
                    const isPast = x <= playheadX;

                    ctx.fillStyle = isPast ? color : `${color}50`;
                    ctx.fillRect(x, H / 2 - barH, Math.max(0.5, barW - 0.25), barH);

                    ctx.fillStyle = isPast ? `${color}70` : `${color}25`;
                    ctx.fillRect(x, H / 2, Math.max(0.5, barW - 0.25), barH);
                }
            }
        } else {
            ctx.fillStyle = "#1a1a20";
            for (let x = 0; x < W; x += 3) {
                const h = (Math.sin(x * 0.15) * 0.3 + 0.3) * H * 0.4;
                ctx.fillRect(x, H / 2 - h / 2, 2, h);
            }
        }

        if (durationMs > 0 && loopRange && loopRange.endMs > loopRange.startMs) {
            const startMs = Math.max(viewStartMs, loopRange.startMs);
            const endMs = Math.min(viewEndMs, loopRange.endMs);
            if (endMs > startMs) {
                const startX = Math.max(0, Math.min(W, msToX(startMs)));
                const endX = Math.max(0, Math.min(W, msToX(endMs)));
                if (endX > startX + 1) {
                    ctx.fillStyle = `${color}22`;
                    ctx.fillRect(startX, 0, endX - startX, H);

                    ctx.strokeStyle = `${color}cc`;
                    ctx.lineWidth = 1.8;
                    ctx.beginPath();
                    ctx.moveTo(startX, 0);
                    ctx.lineTo(startX, H);
                    ctx.moveTo(endX, 0);
                    ctx.lineTo(endX, H);
                    ctx.stroke();

                    const loopLabel = `LOOP ${loopRange.beats ?? ""}`.trim();
                    ctx.font = "bold 10px ui-monospace, SFMono-Regular, Menlo, monospace";
                    const pad = 4;
                    const textW = ctx.measureText(loopLabel).width;
                    const labelX = Math.min(W - textW - pad * 2 - 2, Math.max(2, startX + 2));
                    ctx.fillStyle = "#00000099";
                    ctx.fillRect(labelX, 2, textW + pad * 2, 12);
                    ctx.fillStyle = color;
                    ctx.fillText(loopLabel, labelX + pad, 11);
                }
            }
        }

        if (durationMs > 0) {
            const px = msToX(playPosMs);
            ctx.strokeStyle = playheadColor;
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(px, 0);
            ctx.lineTo(px, H);
            ctx.stroke();

            ctx.fillStyle = playheadColor;
            ctx.beginPath();
            ctx.moveTo(px - 4, 0);
            ctx.lineTo(px + 4, 0);
            ctx.lineTo(px, 6);
            ctx.closePath();
            ctx.fill();
        }

        if (durationMs > 0 && cueMarkers.length > 0) {
            cueMarkers.forEach((cue) => {
                if (cue.positionMs < viewStartMs || cue.positionMs > viewEndMs) return;
                const x = msToX(cue.positionMs);
                if (x < 0 || x > W) return;
                const cueColor = cue.color || "#f59e0b";
                ctx.strokeStyle = cueColor;
                ctx.lineWidth = 2;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, H);
                ctx.stroke();

                ctx.fillStyle = cueColor;
                ctx.beginPath();
                ctx.moveTo(x - 4, 0);
                ctx.lineTo(x + 4, 0);
                ctx.lineTo(x, 6);
                ctx.closePath();
                ctx.fill();

                if (cue.label) {
                    const text = cue.label.trim().slice(0, 6);
                    if (text.length > 0) {
                        ctx.font = "bold 9px ui-monospace, SFMono-Regular, Menlo, monospace";
                        const pad = 3;
                        const tw = ctx.measureText(text).width;
                        const tx = Math.min(W - tw - pad * 2 - 2, Math.max(2, x + 3));
                        ctx.fillStyle = "#000000a8";
                        ctx.fillRect(tx, 8, tw + pad * 2, 10);
                        ctx.fillStyle = cueColor;
                        ctx.fillText(text, tx + pad, 16);
                    }
                }
            });
        }

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
    }, [
        waveformData,
        positionMs,
        animatedPosMs,
        durationMs,
        color,
        playheadColor,
        cueMarkers,
        beatGridMs,
        loopRange,
        canvasWidth,
        height,
        animatePlayhead,
        scrollWithPlayhead,
        scrollWindowMs,
    ]);

    const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
        if (durationMs === 0) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const ratio = (e.clientX - rect.left) / rect.width;
        const targetMs = scrollWithPlayhead
            ? Math.round(viewStartRef.current + ratio * viewDurationRef.current)
            : Math.round(ratio * durationMs);
        const safeMs = Math.max(0, Math.min(durationMs, targetMs));
        if (e.altKey && onAltSetCue) {
            onAltSetCue(safeMs);
            return;
        }
        onSeek?.(safeMs);
    };

    return (
        <div ref={wrapRef} className="waveform-wrap" style={{ height }}>
            <canvas
                ref={canvasRef}
                className="waveform-canvas"
                style={{ width: "100%", height: "100%", cursor: onSeek ? "crosshair" : "default" }}
                onClick={handleClick}
            />
        </div>
    );
}
