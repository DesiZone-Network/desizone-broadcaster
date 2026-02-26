import { useState, useEffect, useRef, useCallback } from "react";
import {
    Play, Pause, Square, SkipBack, SkipForward,
    Volume2, Headphones, Radio, Music2, X,
} from "lucide-react";
import {
    playDeck, pauseDeck, seekDeck, setChannelGain,
    stopDeck, nextDeck,
    setDeckTempo, setDeckLoop, clearDeckLoop,
    getDeckState,
    onDeckStateChanged, onVuMeter,
    getSong, getWaveformData, loadTrack,
    analyzeBeatgrid,
    analyzeStems,
    clearHotCue,
    getBeatgrid,
    getLatestStemAnalysis,
    getStemsRuntimeStatus,
    getHotCues,
    installStemsRuntime,
    renameHotCue,
    recolorHotCue,
    setDeckStemSource,
    setDeckCuePreviewEnabled,
    setHotCue,
    triggerHotCue,
    CueQuantize,
    getChannelDsp,
    setChannelStemFilter,
    StemAnalysis,
    StemFilterMode,
    StemPlaybackSource,
    StemsRuntimeStatus,
    PipelineSettings,
    DeckId, DeckStateEvent, VuEvent, HotCue, BeatGridAnalysis,
} from "../../lib/bridge";
import { writeEventLog } from "../../lib/bridge7";
import type { SamSong } from "../../lib/bridge";
import { WaveformCanvas } from "./WaveformCanvas";
import { VUMeter } from "./VUMeter";
import { parseSongDragPayload } from "../../lib/songDrag";

interface Props {
    deckId: DeckId;
    label: string;
    accentColor?: string;
    isOnAir?: boolean;
    onCollapse?: () => void;
}

function formatTime(ms: number) {
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
}

function VolumeSlider({
    value,
    max = 1.5,
    onChange,
}: {
    value: number;
    max?: number;
    onChange: (v: number) => void;
}) {
    const ref = useRef<HTMLDivElement>(null);
    const dragging = useRef(false);

    const getVal = (e: MouseEvent | React.MouseEvent) => {
        const el = ref.current!;
        const rect = el.getBoundingClientRect();
        return Math.max(0, Math.min(max, ((e.clientX - rect.left) / rect.width) * max));
    };

    const handleMouseDown = (e: React.MouseEvent) => {
        dragging.current = true;
        onChange(getVal(e));
        e.preventDefault();
    };

    useEffect(() => {
        const move = (e: MouseEvent) => { if (dragging.current) onChange(getVal(e)); };
        const up = () => { dragging.current = false; };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
        return () => { window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", up); };
    });

    return (
        <div
            ref={ref}
            className="slider-root"
            style={{ height: 16, cursor: "pointer" }}
            onMouseDown={handleMouseDown}
            role="slider"
            aria-valuenow={Math.round(value * 100)}
            aria-valuemin={0}
            aria-valuemax={Math.round(max * 100)}
        >
            <div className="slider-track" style={{ height: 4 }}>
                <div className="slider-range" style={{ width: `${(value / max) * 100}%` }} />
            </div>
            <div
                className="slider-thumb"
                style={{ position: "absolute", left: `${(value / max) * 100}%`, transform: "translateX(-50%)" }}
            />
        </div>
    );
}

function MarqueeLine({
    text,
    color = "var(--text-primary)",
    fontSize = 12,
    className = "",
}: {
    text: string;
    color?: string;
    fontSize?: number;
    className?: string;
}) {
    const wrapRef = useRef<HTMLDivElement>(null);
    const textRef = useRef<HTMLSpanElement>(null);
    const [overflow, setOverflow] = useState(false);

    useEffect(() => {
        const check = () => {
            const wrap = wrapRef.current;
            const txt = textRef.current;
            if (!wrap || !txt) return;
            setOverflow(txt.scrollWidth > wrap.clientWidth + 4);
        };
        check();
        const id = setTimeout(check, 0);
        window.addEventListener("resize", check);
        return () => {
            clearTimeout(id);
            window.removeEventListener("resize", check);
        };
    }, [text]);

    if (!overflow) {
        return (
            <div
                ref={wrapRef}
                className={className}
                style={{
                    fontSize,
                    color,
                    lineHeight: 1.3,
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                }}
                title={text}
            >
                <span ref={textRef}>{text}</span>
            </div>
        );
    }

    return (
        <div
            ref={wrapRef}
            className={`marquee-line ${className}`}
            style={{ fontSize, color, lineHeight: 1.3 }}
            title={text}
        >
            <span className="marquee-track">{text}</span>
            <span className="marquee-track" aria-hidden="true">{text}</span>
        </div>
    );
}

function filenameFromPath(path?: string | null): string {
    if (!path) return "Track Loaded";
    const base = path.replace(/\\/g, "/").split("/").pop() ?? path;
    return base.replace(/\.[^.]+$/, "");
}

const HOT_CUE_COLORS = [
    "#f59e0b",
    "#06b6d4",
    "#22c55e",
    "#ef4444",
    "#a855f7",
    "#eab308",
    "#14b8a6",
    "#f97316",
];
const HOT_CUE_SLOTS = 4;

function bpmMismatch(a: number | null, b: number | null): number {
    if (!a || !b || a <= 0 || b <= 0) return 0;
    return Math.abs(a - b) / b;
}

function defaultStemAmountForDeck(deckId: DeckId): number {
    return deckId === "deck_a" || deckId === "deck_b" ? 0.82 : 0.70;
}

function isGeneratedStemPath(path: string | null | undefined): boolean {
    if (!path) return false;
    return path.replace(/\\/g, "/").includes("/stems/song_");
}

function detectStemSource(path: string | null | undefined, analysis: StemAnalysis | null): StemPlaybackSource {
    if (!path || !analysis) return "original";
    if (path === analysis.vocals_file_path) return "vocals";
    if (path === analysis.instrumental_file_path) return "instrumental";
    return "original";
}

export function DeckPanel({ deckId, label, accentColor = "#f59e0b", isOnAir = false, onCollapse }: Props) {
    const [deckState, setDeckState] = useState<DeckStateEvent | null>(null);
    const [vuData, setVuData] = useState<VuEvent | null>(null);
    const [volume, setVolume] = useState(1.0);
    const [tempoPct, setTempoPct] = useState(0);
    const [monitorMode, setMonitorMode] = useState<"air" | "cue">("air");
    const [waveformData, setWaveformData] = useState<Float32Array | null>(null);
    const [hotCues, setHotCues] = useState<HotCue[]>([]);
    const [beatgrid, setBeatgrid] = useState<BeatGridAnalysis | null>(null);
    const [cueQuantize, setCueQuantize] = useState<CueQuantize>("off");
    const [selectedCueSlot, setSelectedCueSlot] = useState<number>(1);
    const [beatLoop, setBeatLoop] = useState<{ startMs: number; endMs: number; beats: number } | null>(null);
    const [isFocused, setIsFocused] = useState(false);
    const [isDragOver, setIsDragOver] = useState(false);
    const [loadError, setLoadError] = useState<string | null>(null);
    const [loadedSong, setLoadedSong] = useState<{ title: string; artist: string; path?: string | null } | null>(null);
    const [songMetaBpm, setSongMetaBpm] = useState<number | null>(null);
    const [stemFilter, setStemFilter] = useState<{ mode: StemFilterMode; amount: number }>({
        mode: "off",
        amount: defaultStemAmountForDeck(deckId),
    });
    const [stemAnalysis, setStemAnalysis] = useState<StemAnalysis | null>(null);
    const [stemSource, setStemSource] = useState<StemPlaybackSource>("original");
    const [stemsBusy, setStemsBusy] = useState(false);
    const [stemsStatus, setStemsStatus] = useState<string | null>(null);
    const [runtimeStatus, setRuntimeStatus] = useState<StemsRuntimeStatus | null>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const loadErrorTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
    const panelRef = useRef<HTMLDivElement>(null);
    const originalTrackPathRef = useRef<string | null>(null);

    const isPlaying = deckState?.state === "playing" || deckState?.state === "crossfading";
    const positionMs = deckState?.position_ms ?? 0;
    const durationMs = deckState?.duration_ms ?? 0;
    const remaining = Math.max(0, durationMs - positionMs);
    const analyzedBpm = beatgrid?.bpm && beatgrid.bpm > 0 ? beatgrid.bpm : null;
    const sourceBpm = (() => {
        if (songMetaBpm && analyzedBpm && bpmMismatch(analyzedBpm, songMetaBpm) > 0.08) {
            return songMetaBpm;
        }
        return analyzedBpm ?? songMetaBpm ?? null;
    })();
    const effectiveBpm = sourceBpm ? sourceBpm * (1 + tempoPct / 100) : null;
    const headline = loadedSong
        ? [loadedSong.artist, loadedSong.title].filter(Boolean).join(" - ")
        : "";

    useEffect(() => {
        const unsub = onDeckStateChanged((e) => {
            if (e.deck === deckId) setDeckState(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [deckId]);

    useEffect(() => {
        if (!deckState) return;
        if (typeof deckState.tempo_pct === "number") setTempoPct(deckState.tempo_pct);
        if (typeof deckState.cue_preview_enabled === "boolean") {
            setMonitorMode(deckState.cue_preview_enabled ? "cue" : "air");
        }
    }, [deckState?.tempo_pct, deckState?.cue_preview_enabled]);

    useEffect(() => {
        const unsub = onVuMeter((e) => {
            if (e.channel === deckId) setVuData(e);
        });
        return () => { unsub.then((f) => f()); };
    }, [deckId]);

    useEffect(() => {
        let cancelled = false;
        const fallback = { mode: "off" as StemFilterMode, amount: defaultStemAmountForDeck(deckId) };
        getChannelDsp(deckId)
            .then((row) => {
                if (cancelled) return;
                if (!row?.pipeline_settings_json) {
                    setStemFilter(fallback);
                    return;
                }
                try {
                    const parsed = JSON.parse(row.pipeline_settings_json) as PipelineSettings;
                    const mode = parsed?.stem_filter?.mode ?? "off";
                    const amount = parsed?.stem_filter?.amount ?? fallback.amount;
                    setStemFilter({
                        mode: mode as StemFilterMode,
                        amount: Math.max(0, Math.min(1, amount)),
                    });
                } catch {
                    setStemFilter(fallback);
                }
            })
            .catch(() => {
                if (!cancelled) setStemFilter(fallback);
            });
        return () => {
            cancelled = true;
        };
    }, [deckId]);

    useEffect(() => {
        let cancelled = false;
        getStemsRuntimeStatus()
            .then((status) => {
                if (!cancelled) setRuntimeStatus(status);
            })
            .catch(() => {
                if (!cancelled) setRuntimeStatus(null);
            });
        return () => {
            cancelled = true;
        };
    }, []);

    useEffect(() => {
        let cancelled = false;
        let waveformTimer: ReturnType<typeof setTimeout> | null = null;
        let beatgridTimer: ReturnType<typeof setTimeout> | null = null;
        const songId = deckState?.song_id ?? null;
        const filePath = deckState?.file_path ?? null;

        if (!songId && !filePath) {
            setLoadedSong(null);
            setSongMetaBpm(null);
            setWaveformData(null);
            setHotCues([]);
            setBeatgrid(null);
            setBeatLoop(null);
            setStemAnalysis(null);
            setStemSource("original");
            setStemsStatus(null);
            originalTrackPathRef.current = null;
            return;
        }

        if (filePath && !isGeneratedStemPath(filePath)) {
            originalTrackPathRef.current = filePath;
        }

        if (songId) {
            getSong(songId)
                .then((song) => {
                    if (cancelled || !song) return;
                    setLoadedSong({
                        title: song.title || filenameFromPath(filePath),
                        artist: song.artist || "",
                        path: filePath,
                    });
                    setSongMetaBpm(song.bpm && song.bpm > 0 ? song.bpm : null);
                })
                .catch(() => {
                    if (cancelled) return;
                    setLoadedSong({
                        title: filenameFromPath(filePath),
                        artist: "",
                        path: filePath,
                    });
                    setSongMetaBpm(null);
                });
        } else {
            setLoadedSong({
                title: filenameFromPath(filePath),
                artist: "",
                path: filePath,
            });
            setSongMetaBpm(null);
        }

        if (filePath) {
            setWaveformData(null);
            waveformTimer = setTimeout(() => {
                getWaveformData(filePath, 1400)
                    .then((wf) => {
                        if (!cancelled) setWaveformData(wf);
                    })
                    .catch(() => {
                        if (!cancelled) setWaveformData(null);
                    });
            }, 250);
        } else {
            setWaveformData(null);
        }

        if (songId) {
            getHotCues(songId)
                .then((cues) => {
                    if (!cancelled) setHotCues(cues);
                })
                .catch(() => {
                    if (!cancelled) setHotCues([]);
                });
        } else {
            setHotCues([]);
        }

        if (songId && filePath) {
            getBeatgrid(songId, filePath)
                .then(async (grid) => {
                    if (cancelled) return;
                    if (grid) {
                        setBeatgrid(grid);
                        return;
                    }
                    setBeatgrid(null);
                    beatgridTimer = setTimeout(async () => {
                        try {
                            const analyzed = await analyzeBeatgrid(songId, filePath, false);
                            if (!cancelled) setBeatgrid(analyzed);
                        } catch {
                            if (!cancelled) setBeatgrid(null);
                        }
                    }, 1200);
                })
                .catch(() => {
                    if (!cancelled) setBeatgrid(null);
                });
        } else {
            setBeatgrid(null);
        }

        if (songId) {
            getLatestStemAnalysis(songId)
                .then((analysis) => {
                    if (cancelled) return;
                    if (!analysis) {
                        setStemAnalysis(null);
                        if (filePath && !isGeneratedStemPath(filePath)) {
                            setStemSource("original");
                        }
                        return;
                    }
                    setStemAnalysis(analysis);
                    originalTrackPathRef.current = analysis.source_file_path;
                    setStemSource(detectStemSource(filePath, analysis));
                })
                .catch(() => {
                    if (!cancelled) {
                        setStemAnalysis(null);
                        if (filePath && !isGeneratedStemPath(filePath)) {
                            setStemSource("original");
                        }
                    }
                });
        } else {
            setStemAnalysis(null);
            if (filePath && !isGeneratedStemPath(filePath)) {
                setStemSource("original");
            }
        }

        return () => {
            cancelled = true;
            if (waveformTimer) clearTimeout(waveformTimer);
            if (beatgridTimer) clearTimeout(beatgridTimer);
        };
    }, [deckState?.song_id, deckState?.file_path]);

    const handleVolumeChange = useCallback((v: number) => {
        setVolume(v);
        setChannelGain(deckId, v).catch(console.error);
    }, [deckId]);

    const handleTempoChange = useCallback((v: number) => {
        setTempoPct(v);
        setDeckTempo(deckId, v).catch(console.error);
    }, [deckId]);

    const handleSeek = useCallback((ms: number) => {
        seekDeck(deckId, ms).catch(console.error);
    }, [deckId]);

    const applyStemFilter = useCallback(async (mode: StemFilterMode, amount?: number) => {
        const nextAmount = Math.max(0, Math.min(1, amount ?? stemFilter.amount));
        try {
            await setChannelStemFilter(deckId, mode, nextAmount);
            setStemFilter({ mode, amount: nextAmount });
        } catch (err) {
            console.error(err);
        }
    }, [deckId, stemFilter.amount]);

    const applyStemPreset = useCallback((preset: "off" | "vocal_boost" | "karaoke" | "light") => {
        switch (preset) {
            case "off":
                applyStemFilter("off");
                break;
            case "vocal_boost":
                applyStemFilter("vocal", 0.72);
                break;
            case "karaoke":
                applyStemFilter("instrumental", 0.90);
                break;
            case "light":
                applyStemFilter("instrumental", 0.55);
                break;
            default:
                break;
        }
    }, [applyStemFilter]);

    const generateStemsForCurrentTrack = useCallback(async (force = false) => {
        const songId = deckState?.song_id ?? null;
        const currentPath = deckState?.file_path ?? null;
        if (!songId) {
            setStemsStatus("Load a library track with song ID first.");
            return;
        }
        const basePath =
            originalTrackPathRef.current ??
            stemAnalysis?.source_file_path ??
            (currentPath && !isGeneratedStemPath(currentPath) ? currentPath : null);
        if (!basePath) {
            setStemsStatus("Original source path not available.");
            return;
        }
        setStemsBusy(true);
        try {
            const currentRuntime = runtimeStatus ?? (await getStemsRuntimeStatus());
            setRuntimeStatus(currentRuntime);
            if (!currentRuntime.ready) {
                setStemsStatus("Installing stems runtime… first run takes longer.");
                const installed = await installStemsRuntime();
                setRuntimeStatus(installed);
                if (!installed.ready) {
                    setStemsStatus(installed.message);
                    return;
                }
            }
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            setStemsStatus(msg);
            setStemsBusy(false);
            return;
        }
        setStemsStatus("Generating stems… this may take some time.");
        try {
            const analysis = await analyzeStems(songId, basePath, force);
            setStemAnalysis(analysis);
            originalTrackPathRef.current = analysis.source_file_path;
            setStemsStatus("Stems ready. Use VOCALS or INSTRUMENTAL.");
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            setStemsStatus(msg);
        } finally {
            setStemsBusy(false);
        }
    }, [deckState?.file_path, deckState?.song_id, runtimeStatus, stemAnalysis?.source_file_path]);

    const installRuntimeOnly = useCallback(async () => {
        setStemsBusy(true);
        setStemsStatus("Installing stems runtime…");
        try {
            const status = await installStemsRuntime();
            setRuntimeStatus(status);
            setStemsStatus(status.message);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            setStemsStatus(msg);
        } finally {
            setStemsBusy(false);
        }
    }, []);

    const applyStemSource = useCallback(async (source: StemPlaybackSource) => {
        const songId = deckState?.song_id ?? null;
        if (!songId) {
            setStemsStatus("No track loaded.");
            return;
        }
        const originalPath =
            originalTrackPathRef.current ??
            stemAnalysis?.source_file_path ??
            (deckState?.file_path && !isGeneratedStemPath(deckState.file_path) ? deckState.file_path : undefined);
        setStemsBusy(true);
        setStemsStatus(source === "original" ? "Switching to original…" : `Switching to ${source}…`);
        try {
            const result = await setDeckStemSource(deckId, source, songId, originalPath);
            setStemSource(result.source);
            if (result.source === "original") {
                originalTrackPathRef.current = result.file_path;
            }
            if (result.source !== "original") {
                await setChannelStemFilter(deckId, "off", stemFilter.amount);
                setStemFilter((prev) => ({ ...prev, mode: "off" }));
            }
            setStemsStatus(null);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            setStemsStatus(msg);
        } finally {
            setStemsBusy(false);
        }
    }, [deckId, deckState?.file_path, deckState?.song_id, stemAnalysis?.source_file_path, stemFilter.amount]);

    const buildBeatLoopRange = useCallback((beats: number) => {
        if (beats <= 0) return null;
        if (durationMs <= 0) return null;

        if (beatgrid?.beat_times_ms && beatgrid.beat_times_ms.length >= 2) {
            let beatsMs = beatgrid.beat_times_ms;
            const analyzedBpm = beatgrid.bpm > 0 ? beatgrid.bpm : 0;
            const metaBpm = songMetaBpm && songMetaBpm > 0 ? songMetaBpm : 0;
            if (analyzedBpm > 0 && metaBpm > 0) {
                if (Math.abs(metaBpm - analyzedBpm) / metaBpm > 0.08) {
                    // If analyzer tempo disagrees with trusted metadata, synthesize
                    // a loop grid at metadata BPM anchored to analyzed first beat.
                    const periodMs = 60_000 / metaBpm;
                    const anchorMs = Math.max(0, beatgrid.first_beat_ms || beatsMs[0] || 0);
                    const posBeats = (positionMs - anchorMs) / periodMs;
                    const quantizedBeatIndex = isPlaying
                        ? Math.ceil(posBeats - 1e-6)
                        : Math.round(posBeats);
                    const startMs = Math.max(0, Math.round(anchorMs + quantizedBeatIndex * periodMs));
                    const endMs = Math.min(durationMs, Math.round(startMs + beats * periodMs));
                    if (endMs <= startMs + 25) return null;
                    return { startMs, endMs };
                }
                const ratio = metaBpm / analyzedBpm;
                if (ratio >= 1.8 && ratio <= 2.2) {
                    // Analyzer chose half-time: insert midpoint beats.
                    const expanded: number[] = [];
                    for (let i = 0; i < beatsMs.length - 1; i += 1) {
                        const a = beatsMs[i];
                        const b = beatsMs[i + 1];
                        expanded.push(a, Math.round((a + b) * 0.5));
                    }
                    expanded.push(beatsMs[beatsMs.length - 1]);
                    beatsMs = expanded;
                } else if (ratio >= 0.45 && ratio <= 0.55) {
                    // Analyzer chose double-time: keep every 2nd beat.
                    beatsMs = beatsMs.filter((_, idx) => idx % 2 === 0);
                    if (beatsMs.length < 2) {
                        beatsMs = beatgrid.beat_times_ms;
                    }
                }
            }
            let startIdx = 0;
            if (isPlaying) {
                // Serato-like quantized engage: loop starts on the next beat boundary.
                let nextIdx = beatsMs.findIndex((ms) => ms >= positionMs);
                if (nextIdx < 0) nextIdx = beatsMs.length - 1;
                startIdx = Math.max(0, nextIdx);
            } else {
                for (let i = 0; i < beatsMs.length; i += 1) {
                    if (beatsMs[i] <= positionMs) startIdx = i;
                    else break;
                }
                if (startIdx + 1 < beatsMs.length) {
                    const cur = beatsMs[startIdx];
                    const nxt = beatsMs[startIdx + 1];
                    if (Math.abs(nxt - positionMs) < Math.abs(positionMs - cur)) {
                        startIdx += 1;
                    }
                }
            }
            const startMs = Math.max(0, beatsMs[startIdx] ?? 0);
            let endMs = beatsMs[startIdx + beats];
            if (endMs == null) {
                const fallbackBeatMs =
                    beatgrid.bpm > 0 ? Math.round((60_000 / beatgrid.bpm) * beats) : 500 * beats;
                endMs = Math.min(durationMs, startMs + fallbackBeatMs);
            }
            if (endMs <= startMs + 25) return null;
            return { startMs, endMs };
        }

        const bpm = beatgrid?.bpm && beatgrid.bpm > 0 ? beatgrid.bpm : 120;
        const beatMs = Math.round(60_000 / bpm);
        const loopMs = beatMs * beats;
        const startMs = Math.max(0, positionMs);
        const endMs = Math.min(durationMs, startMs + loopMs);
        if (endMs <= startMs + 25) return null;
        return { startMs, endMs };
    }, [beatgrid, durationMs, isPlaying, positionMs, songMetaBpm]);

    const activateBeatLoop = useCallback((beats: number) => {
        const range = buildBeatLoopRange(beats);
        if (!range) return;
        setBeatLoop({ startMs: range.startMs, endMs: range.endMs, beats });
        setDeckLoop(deckId, range.startMs, range.endMs)
            .then(() => seekDeck(deckId, range.startMs))
            .catch(console.error);
    }, [buildBeatLoopRange, deckId]);

    const clearBeatLoop = useCallback(() => {
        setBeatLoop(null);
        clearDeckLoop(deckId).catch(console.error);
    }, [deckId]);

    const syncToOtherDeck = useCallback(async () => {
        const ownBpm = beatgrid?.bpm ?? 0;
        if (!(ownBpm > 0)) return;
        const otherDeckId: DeckId = deckId === "deck_a" ? "deck_b" : "deck_a";
        const other = await getDeckState(otherDeckId);
        if (!other?.song_id || !other?.file_path) return;
        const otherGrid = await getBeatgrid(other.song_id, other.file_path);
        const targetBpm = otherGrid?.bpm ?? 0;
        if (!(targetBpm > 0)) return;
        const pct = Math.max(-50, Math.min(50, ((targetBpm / ownBpm) - 1) * 100));
        setTempoPct(pct);
        await setDeckTempo(deckId, pct);
    }, [beatgrid?.bpm, deckId]);

    const handlePlay = async () => {
        try {
            if (isPlaying) await pauseDeck(deckId);
            else await playDeck(deckId);
        } catch (e) { console.error(e); }
    };

    const handleStop = async () => {
        try {
            await stopDeck(deckId);
        } catch (e) { console.error(e); }
    };

    const refreshHotCues = useCallback(async () => {
        const songId = deckState?.song_id ?? null;
        if (!songId) {
            setHotCues([]);
            return;
        }
        try {
            const cues = await getHotCues(songId);
            setHotCues(cues);
        } catch {
            setHotCues([]);
        }
    }, [deckState?.song_id]);

    const setCueAtPosition = useCallback(async (slot: number, atMs: number) => {
        const songId = deckState?.song_id ?? null;
        if (!songId) return;
        const existing = hotCues.find((c) => c.slot === slot);
        const color = existing?.color_hex ?? HOT_CUE_COLORS[(slot - 1) % HOT_CUE_COLORS.length];
        const label = existing?.label ?? `Cue ${slot}`;
        try {
            const cue = await setHotCue(songId, slot, atMs, label, color, cueQuantize);
            setHotCues((prev) => {
                const next = prev.filter((c) => c.slot !== slot);
                next.push(cue);
                next.sort((a, b) => a.slot - b.slot);
                return next;
            });
        } catch (err) {
            console.error(err);
        }
    }, [cueQuantize, deckState?.song_id, hotCues]);

    const triggerOrSetCue = useCallback(async (slot: number, forceSet = false) => {
        const songId = deckState?.song_id ?? null;
        if (!songId) return;
        const existing = hotCues.find((c) => c.slot === slot);
        if (forceSet || !existing) {
            await setCueAtPosition(slot, positionMs);
            return;
        }
        try {
            const jumped = await triggerHotCue(deckId, songId, slot, cueQuantize);
            if (jumped.quantized) {
                setHotCues((prev) => prev.map((c) => (c.slot === slot ? { ...c, position_ms: jumped.position_ms } : c)));
            }
        } catch (err) {
            console.error(err);
        }
    }, [cueQuantize, deckId, deckState?.song_id, hotCues, positionMs, setCueAtPosition]);

    const clearCueSlot = useCallback(async (slot: number) => {
        const songId = deckState?.song_id ?? null;
        if (!songId) return;
        try {
            await clearHotCue(songId, slot);
            setHotCues((prev) => prev.filter((c) => c.slot !== slot));
        } catch (err) {
            console.error(err);
        }
    }, [deckState?.song_id]);

    const renameCueSlot = useCallback(async (slot: number) => {
        const songId = deckState?.song_id ?? null;
        if (!songId) return;
        const current = hotCues.find((c) => c.slot === slot)?.label ?? `Cue ${slot}`;
        const next = window.prompt("Cue label", current);
        if (next == null) return;
        try {
            await renameHotCue(songId, slot, next);
            await refreshHotCues();
        } catch (err) {
            console.error(err);
        }
    }, [deckState?.song_id, hotCues, refreshHotCues]);

    const recolorCueSlot = useCallback(async (slot: number, colorHex: string) => {
        const songId = deckState?.song_id ?? null;
        if (!songId) return;
        try {
            await recolorHotCue(songId, slot, colorHex);
            setHotCues((prev) => prev.map((c) => (c.slot === slot ? { ...c, color_hex: colorHex } : c)));
        } catch (err) {
            console.error(err);
        }
    }, [deckState?.song_id]);

    const applyMonitorMode = useCallback(async (mode: "air" | "cue") => {
        try {
            await setDeckCuePreviewEnabled(deckId, mode === "cue");
            setMonitorMode(mode);
        } catch (err) {
            console.error(err);
            setMonitorMode("air");
        }
    }, [deckId]);

    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            if (!isFocused) return;
            const target = e.target as HTMLElement | null;
            if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
            const slot = parseInt(e.key, 10);
            if (!Number.isInteger(slot) || slot < 1 || slot > HOT_CUE_SLOTS) return;
            setSelectedCueSlot(slot);
            e.preventDefault();
            triggerOrSetCue(slot, e.shiftKey).catch(console.error);
        };
        window.addEventListener("keydown", onKeyDown);
        return () => window.removeEventListener("keydown", onKeyDown);
    }, [isFocused, triggerOrSetCue]);

    const showLoadError = (msg: string) => {
        setLoadError(msg);
        if (loadErrorTimer.current) clearTimeout(loadErrorTimer.current);
        loadErrorTimer.current = setTimeout(() => setLoadError(null), 6000);
    };

    const handleLoadFile = () => fileInputRef.current?.click();

    const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const filePath = (file as any).path ?? file.name;
        try {
            await loadTrack(deckId, filePath);
            setLoadError(null);
            setLoadedSong({ title: filenameFromPath(filePath), artist: "", path: filePath });
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            showLoadError(msg);
            setLoadedSong(null);
            writeEventLog({
                level: "error",
                category: "audio",
                event: "track_load_failed",
                message: `Failed to load file "${file.name}" on ${deckId}: ${msg}`,
                deck: deckId,
            }).catch(() => {});
        }
        e.target.value = "";
    };

    return (
        <div
            ref={panelRef}
            className="deck-panel"
            tabIndex={0}
            onFocus={() => setIsFocused(true)}
            onBlur={() => setIsFocused(false)}
            style={{
                borderColor: isOnAir ? accentColor + "70" : "var(--border-default)",
                boxShadow: isOnAir ? `0 0 24px ${accentColor}20` : "none",
                flex: 1,
            }}
        >
            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <div
                        style={{
                            width: 6, height: 6, borderRadius: "50%",
                            background: isPlaying ? accentColor : "var(--text-muted)",
                            boxShadow: isPlaying ? `0 0 8px ${accentColor}` : "none",
                            transition: "all 0.2s",
                        }}
                    />
                    <span
                        className="font-semibold tracking-widest uppercase"
                        style={{ fontSize: 11, color: isPlaying ? accentColor : "var(--text-secondary)" }}
                    >
                        {label}
                    </span>
                    {isOnAir && (
                        <span className="badge badge-on-air" style={{ fontSize: 8 }}>LIVE</span>
                    )}
                </div>
                <div className="flex items-center gap-1">
                    <button
                        className="btn btn-ghost"
                        style={{ padding: "3px 8px", fontSize: 10 }}
                        onClick={handleLoadFile}
                        title="Load track"
                    >
                        <Music2 size={11} />
                        LOAD
                    </button>
                    {onCollapse && (
                        <button
                            className="btn btn-ghost btn-icon"
                            style={{ width: 20, height: 20, opacity: 0.4 }}
                            onClick={onCollapse}
                            title="Collapse panel"
                        >
                            <X size={11} />
                        </button>
                    )}
                    <input
                        ref={fileInputRef}
                        type="file"
                        accept="audio/*"
                        style={{ display: "none" }}
                        onChange={handleFileChange}
                    />
                </div>
            </div>

            {/* Track Info — also a DnD drop target */}
            <div
                style={{
                    background: isDragOver ? `${accentColor}12` : "var(--bg-input)",
                    border: isDragOver
                        ? `1px dashed ${accentColor}`
                        : "1px solid var(--border-default)",
                    borderRadius: "var(--r-md)",
                    padding: "8px 10px",
                    minHeight: 48,
                    transition: "border-color 0.15s, background 0.15s",
                }}
                onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; setIsDragOver(true); }}
                onDragLeave={() => setIsDragOver(false)}
                onDrop={async (e) => {
                    e.preventDefault();
                    setIsDragOver(false);
                    const raw = e.dataTransfer.getData("text/plain");
                    if (!raw) return;
                    const song = parseSongDragPayload(raw) as SamSong | null;
                    if (!song) return;
                    try {
                        await loadTrack(deckId, song.filename, song.id);
                        setLoadError(null);
                        setLoadedSong({ title: song.title, artist: song.artist, path: song.filename });
                    } catch (err) {
                        const msg = err instanceof Error ? err.message : String(err);
                        showLoadError(msg);
                        setLoadedSong(null);
                        writeEventLog({
                            level: "error",
                            category: "audio",
                            event: "track_load_failed",
                            message: `Failed to load "${song.artist} – ${song.title}" on ${deckId}: ${msg}`,
                            deck: deckId,
                            songId: song.id,
                        }).catch(() => {});
                    }
                }}
            >
                {loadError ? (
                    <div className="flex items-center gap-2" style={{ minHeight: 32 }}>
                        <span style={{ fontSize: 10, color: "#ef4444", lineHeight: 1.4, wordBreak: "break-all" }}>
                            ⚠ {loadError}
                        </span>
                    </div>
                ) : deckState && deckState.state !== "idle" ? (
                    <div style={{ overflow: "hidden" }}>
                        <MarqueeLine
                            text={
                                headline ||
                                (deckState.state === "playing" || deckState.state === "crossfading"
                                    ? "Playing…"
                                    : "Track Loaded")
                            }
                            className="font-medium"
                            fontSize={12}
                            color="var(--text-primary)"
                        />
                        <MarqueeLine
                            text={loadedSong?.path ? filenameFromPath(loadedSong.path) : deckState.file_path ? filenameFromPath(deckState.file_path) : deckState.state}
                            className="text-xs text-muted"
                            fontSize={10}
                            color="var(--text-muted)"
                        />
                    </div>
                ) : (
                    <div className="flex items-center gap-2 text-muted" style={{ height: 32 }}>
                        <Music2 size={14} />
                        <span style={{ fontSize: 11 }}>
                            {isDragOver ? "Drop to load track" : "No track loaded — drag here or click LOAD"}
                        </span>
                    </div>
                )}
            </div>

            {/* Waveform */}
            <WaveformCanvas
                waveformData={waveformData}
                positionMs={positionMs}
                durationMs={durationMs}
                isPlaying={isPlaying}
                playbackRate={deckState?.playback_rate ?? 1}
                animatePlayhead
                onSeek={handleSeek}
                cueMarkers={hotCues.map((c) => ({ positionMs: c.position_ms, color: c.color_hex, label: c.label }))}
                beatGridMs={beatgrid?.beat_times_ms ?? null}
                loopRange={beatLoop}
                onAltSetCue={(ms) => {
                    const songId = deckState?.song_id ?? null;
                    if (!songId) return;
                    setCueAtPosition(selectedCueSlot, ms).catch(console.error);
                }}
                height={44}
                color={accentColor}
            />

            {/* Time display */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-1">
                    <span className="mono" style={{ fontSize: 18, fontWeight: 600, color: "var(--text-primary)", letterSpacing: "0.04em" }}>
                        {formatTime(positionMs)}
                    </span>
                    <span className="text-muted" style={{ fontSize: 10, marginTop: 3 }}>pos</span>
                </div>
                <div className="flex items-center gap-1">
                    <span className="text-muted" style={{ fontSize: 10, marginTop: 3 }}>rem</span>
                    <span className="mono font-medium" style={{ fontSize: 13, color: remaining < 30000 && remaining > 0 ? "#ef4444" : "var(--text-secondary)", letterSpacing: "0.04em" }}>
                        −{formatTime(remaining)}
                    </span>
                </div>
                <span className="mono text-muted" style={{ fontSize: 10 }}>
                    {formatTime(durationMs)}
                </span>
            </div>

            <div className="flex items-center justify-between" style={{ marginTop: 2 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <div
                        title="Platter"
                        style={{
                            width: 20,
                            height: 20,
                            borderRadius: "50%",
                            border: "1px solid var(--border-strong)",
                            background: "radial-gradient(circle at 35% 35%, #fff 0%, #ddd 18%, #111 20%, #0a0a0d 100%)",
                            position: "relative",
                            transform: `rotate(${(positionMs / 1000) * 220}deg)`,
                            transition: isPlaying ? "none" : "transform 160ms ease-out",
                        }}
                    >
                        <div
                            style={{
                                position: "absolute",
                                left: "50%",
                                top: 1,
                                transform: "translateX(-50%)",
                                width: 2,
                                height: 5,
                                background: accentColor,
                                borderRadius: 1,
                            }}
                        />
                    </div>
                    <span className="mono text-muted" style={{ fontSize: 9 }}>
                        SRC {sourceBpm ? sourceBpm.toFixed(2) : "--"} BPM
                    </span>
                </div>
            </div>

            {/* Transport */}
            <div className="flex items-center gap-1">
                <button className="transport-btn" style={{ width: 28, height: 28 }} onClick={() => seekDeck(deckId, 0)} title="Return to start">
                    <SkipBack size={11} />
                </button>
                <button
                    className={`transport-btn play ${isPlaying ? "stop" : ""}`}
                    onClick={handlePlay}
                    title={isPlaying ? "Pause" : "Play"}
                    style={isPlaying ? {
                        background: "var(--red-glow)",
                        borderColor: "var(--red-dim)",
                        color: "var(--red)",
                        width: 32, height: 32,
                    } : { width: 32, height: 32 }}
                >
                    {isPlaying ? <Pause size={14} /> : <Play size={14} />}
                </button>
                <button className="transport-btn stop" style={{ width: 28, height: 28 }} onClick={handleStop} title="Stop">
                    <Square size={11} />
                </button>
                <button
                    className="transport-btn"
                    style={{ width: 28, height: 28 }}
                    title="Next track"
                    onClick={() => nextDeck(deckId).catch(console.error)}
                >
                    <SkipForward size={11} />
                </button>

                <div style={{ flex: 1 }} />

                {/* VU */}
                <VUMeter vuData={vuData} height={26} width={20} compact />
            </div>

            <div className="flex items-center gap-1" style={{ marginTop: 2, flexWrap: "wrap" }}>
                <button
                    className="btn btn-ghost"
                    style={{ fontSize: 9, minHeight: 16, padding: "0 6px" }}
                    onClick={() => setCueAtPosition(selectedCueSlot, positionMs).catch(console.error)}
                    title={`Set Cue ${selectedCueSlot} at current position`}
                >
                    CUE SET
                </button>
                <button
                    className="btn btn-ghost"
                    style={{ fontSize: 9, minHeight: 16, padding: "0 6px" }}
                    onClick={() => triggerOrSetCue(selectedCueSlot, false).catch(console.error)}
                    title={`Jump to Cue ${selectedCueSlot}`}
                >
                    CUE GO
                </button>
                <button
                    className="btn btn-ghost"
                    style={{ fontSize: 9, minHeight: 16, padding: "0 6px" }}
                    onClick={() => syncToOtherDeck().catch(console.error)}
                    title="Sync this deck BPM to the other deck"
                >
                    SYNC
                </button>
                <span className="mono text-muted" style={{ fontSize: 8 }}>
                    BPM {effectiveBpm ? effectiveBpm.toFixed(2) : "--"}
                </span>
                <span className="mono text-muted" style={{ fontSize: 8, marginLeft: 4 }}>Q</span>
                <select
                    value={cueQuantize}
                    onChange={(e) => setCueQuantize(e.target.value as CueQuantize)}
                    style={{
                        background: "var(--bg-input)",
                        color: "var(--text-secondary)",
                        border: "1px solid var(--border-strong)",
                        borderRadius: 4,
                        fontSize: 9,
                        padding: "1px 4px",
                    }}
                >
                    <option value="off">Off</option>
                    <option value="beat_1">1 Beat</option>
                    <option value="beat_half">1/2 Beat</option>
                    <option value="beat_quarter">1/4 Beat</option>
                </select>
                <span className="mono text-muted" style={{ fontSize: 8, marginLeft: 4 }}>
                    LOOP {beatLoop ? `(${beatLoop.beats}B)` : "(OFF)"}
                </span>
                {[1, 2, 4, 8, 16].map((beats) => (
                    <button
                        key={beats}
                        className="btn"
                        onClick={() => activateBeatLoop(beats)}
                        style={{
                            minHeight: 16,
                            padding: "0 4px",
                            fontSize: 8,
                            border: beatLoop?.beats === beats
                                ? `1px solid ${accentColor}`
                                : "1px solid var(--border-strong)",
                            background: beatLoop?.beats === beats
                                ? `${accentColor}25`
                                : "var(--bg-input)",
                            color: beatLoop?.beats === beats
                                ? "var(--text-primary)"
                                : "var(--text-muted)",
                        }}
                        title={`Loop ${beats} beat${beats > 1 ? "s" : ""}`}
                    >
                        {beats}
                    </button>
                ))}
                <button
                    className="btn"
                    onClick={clearBeatLoop}
                    style={{
                        minHeight: 16,
                        padding: "0 4px",
                        fontSize: 8,
                        border: "1px solid var(--border-strong)",
                        background: "var(--bg-input)",
                        color: "var(--text-muted)",
                    }}
                    title="Disable loop"
                >
                    OFF
                </button>
            </div>

            <div className="flex items-center gap-1" style={{ marginTop: 2, flexWrap: "wrap" }}>
                <span className="mono text-muted" style={{ fontSize: 8 }}>
                    STEM {stemSource !== "original" ? `(${stemSource.toUpperCase()})` : "(OFF)"}
                </span>
                <button
                    className="btn"
                    onClick={() => installRuntimeOnly().catch(console.error)}
                    style={{
                        minHeight: 16,
                        padding: "0 4px",
                        fontSize: 8,
                        border: "1px solid var(--border-strong)",
                        background: runtimeStatus?.ready ? "var(--bg-input)" : `${accentColor}20`,
                        color: runtimeStatus?.ready ? "var(--text-muted)" : "var(--text-primary)",
                    }}
                    disabled={stemsBusy}
                    title={runtimeStatus?.ready ? "Stems runtime installed" : "Install stems runtime"}
                >
                    {runtimeStatus?.ready ? "READY" : "SETUP"}
                </button>
                <button
                    className="btn"
                    onClick={() => generateStemsForCurrentTrack(false).catch(console.error)}
                    style={{
                        minHeight: 16,
                        padding: "0 4px",
                        fontSize: 8,
                        border: "1px solid var(--border-strong)",
                        background: stemsBusy ? "var(--bg-hover)" : "var(--bg-input)",
                        color: "var(--text-muted)",
                    }}
                    disabled={stemsBusy}
                    title="Generate real stems (Demucs)"
                >
                    GEN
                </button>
                {(
                    [
                        { key: "original", label: "ORIG" },
                        { key: "vocals", label: "VOCAL" },
                        { key: "instrumental", label: "INST" },
                    ] as const
                ).map((opt) => {
                    const active = stemSource === opt.key;
                    const unavailable = opt.key !== "original" && !stemAnalysis;
                    return (
                        <button
                            key={opt.key}
                            className="btn"
                            onClick={() => applyStemSource(opt.key).catch(console.error)}
                            style={{
                                minHeight: 16,
                                padding: "0 4px",
                                fontSize: 8,
                                border: active
                                    ? `1px solid ${accentColor}`
                                    : "1px solid var(--border-strong)",
                                background: active
                                    ? `${accentColor}25`
                                    : "var(--bg-input)",
                                color: unavailable
                                    ? "var(--text-dim)"
                                    : active
                                        ? "var(--text-primary)"
                                        : "var(--text-muted)",
                                opacity: unavailable ? 0.55 : 1,
                            }}
                            disabled={stemsBusy || unavailable}
                            title={
                                opt.key === "original"
                                    ? "Play original source"
                                    : !stemAnalysis
                                        ? "Generate stems first"
                                        : `Play ${opt.label.toLowerCase()} stem`
                            }
                        >
                            {opt.label}
                        </button>
                    );
                })}
                <span className="mono text-muted" style={{ fontSize: 8, marginLeft: 4 }}>
                    FLT
                </span>
                {(
                    [
                        { key: "off", label: "OFF" },
                        { key: "vocal_boost", label: "VOCAL" },
                        { key: "karaoke", label: "KARAOKE" },
                        { key: "light", label: "LIGHT" },
                    ] as const
                ).map((p) => {
                    const active =
                        (p.key === "off" && stemFilter.mode === "off") ||
                        (p.key === "vocal_boost" && stemFilter.mode === "vocal") ||
                        (p.key === "karaoke" && stemFilter.mode === "instrumental" && stemFilter.amount >= 0.8) ||
                        (p.key === "light" && stemFilter.mode === "instrumental" && stemFilter.amount < 0.8);
                    return (
                        <button
                            key={p.key}
                            className="btn"
                            onClick={() => applyStemPreset(p.key)}
                            style={{
                                minHeight: 16,
                                padding: "0 4px",
                                fontSize: 8,
                                border: active
                                    ? `1px solid ${accentColor}`
                                    : "1px solid var(--border-strong)",
                                background: active
                                    ? `${accentColor}25`
                                    : "var(--bg-input)",
                                color: active
                                    ? "var(--text-primary)"
                                    : "var(--text-muted)",
                            }}
                            title={`Set ${p.label} filter`}
                        >
                            {p.label}
                        </button>
                    );
                })}
            </div>

            {stemsStatus && (
                <div style={{ marginTop: 1 }}>
                    <span className="mono text-muted" style={{ fontSize: 8, color: "var(--text-muted)" }}>
                        {stemsStatus}
                    </span>
                </div>
            )}

            <div
                style={{
                    display: "grid",
                    gridTemplateColumns: `repeat(${HOT_CUE_SLOTS}, minmax(0, 1fr))`,
                    gap: 3,
                    marginTop: 3,
                }}
            >
                {Array.from({ length: HOT_CUE_SLOTS }).map((_, i) => {
                    const slot = i + 1;
                    const cue = hotCues.find((c) => c.slot === slot);
                    const cueColor = cue?.color_hex ?? HOT_CUE_COLORS[i];
                    return (
                        <div key={slot} style={{ display: "flex", flexDirection: "column", gap: 1 }}>
                            <button
                                className="btn"
                                onClick={(e) => {
                                    setSelectedCueSlot(slot);
                                    triggerOrSetCue(slot, e.shiftKey).catch(console.error);
                                }}
                                onContextMenu={(e) => {
                                    e.preventDefault();
                                    clearCueSlot(slot).catch(console.error);
                                }}
                                onDoubleClick={() => renameCueSlot(slot).catch(console.error)}
                                style={{
                                    minHeight: 20,
                                    padding: "1px 3px",
                                    border: selectedCueSlot === slot
                                        ? `1px solid ${cueColor}`
                                        : "1px solid var(--border-strong)",
                                    background: cue ? `${cueColor}25` : "var(--bg-input)",
                                    color: cue ? "var(--text-primary)" : "var(--text-muted)",
                                    fontSize: 9,
                                    fontWeight: 700,
                                }}
                                title={cue
                                    ? `Cue ${slot}: ${cue.label || "Jump"} (${formatTime(cue.position_ms)})`
                                    : `Set Cue ${slot} at current position`}
                            >
                                {slot}
                            </button>
                            <div className="flex items-center gap-1" style={{ minHeight: 10 }}>
                                <input
                                    type="color"
                                    value={cueColor}
                                    onChange={(e) => recolorCueSlot(slot, e.target.value).catch(console.error)}
                                    style={{
                                        width: 10,
                                        height: 10,
                                        padding: 0,
                                        border: "none",
                                        background: "transparent",
                                    }}
                                    title={`Cue ${slot} color`}
                                />
                                <button
                                    className="btn btn-ghost"
                                    onClick={() => renameCueSlot(slot).catch(console.error)}
                                    style={{
                                        padding: "0 2px",
                                        minHeight: 10,
                                        fontSize: 8,
                                        color: "var(--text-muted)",
                                    }}
                                    title={`Rename Cue ${slot}`}
                                >
                                    {cue?.label?.slice(0, 3) || `C${slot}`}
                                </button>
                            </div>
                        </div>
                    );
                })}
            </div>

            {/* Volume + Monitor */}
            <div className="flex items-center gap-3">
                <Volume2 size={12} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                <div style={{ flex: 1 }}>
                    <VolumeSlider value={volume} max={1.5} onChange={handleVolumeChange} />
                </div>
                <span className="mono" style={{ fontSize: 10, color: accentColor, minWidth: 28 }}>
                    {Math.round(volume * 100)}%
                </span>
                <button className="btn btn-ghost btn-icon" style={{ width: 18, height: 18 }} title="Reset volume to 100%" onClick={() => handleVolumeChange(1)}>
                    ↺
                </button>

                {/* Air / Cue toggle */}
                <div className="flex" style={{ border: "1px solid var(--border-strong)", borderRadius: "var(--r-md)", overflow: "hidden" }}>
                    <button
                        onClick={() => applyMonitorMode("air").catch(console.error)}
                        style={{
                            padding: "4px 8px",
                            fontSize: 10,
                            fontWeight: 600,
                            letterSpacing: "0.08em",
                            border: "none",
                            cursor: "pointer",
                            background: monitorMode === "air" ? accentColor : "var(--bg-input)",
                            color: monitorMode === "air" ? "#000" : "var(--text-muted)",
                            transition: "all 0.15s",
                        }}
                    >
                        <Radio size={10} style={{ display: "inline", marginRight: 3 }} />AIR
                    </button>
                    <button
                        onClick={() => applyMonitorMode("cue").catch(console.error)}
                        style={{
                            padding: "4px 8px",
                            fontSize: 10,
                            fontWeight: 600,
                            letterSpacing: "0.08em",
                            border: "none",
                            borderLeft: "1px solid var(--border-strong)",
                            cursor: "pointer",
                            background: monitorMode === "cue" ? "var(--cyan)" : "var(--bg-input)",
                            color: monitorMode === "cue" ? "#000" : "var(--text-muted)",
                            transition: "all 0.15s",
                        }}
                    >
                        <Headphones size={10} style={{ display: "inline", marginRight: 3 }} />CUE
                    </button>
                </div>
            </div>

            <div className="flex items-center gap-2" style={{ marginTop: 2 }}>
                <span className="mono text-muted" style={{ fontSize: 9, minWidth: 36 }}>TEMPO</span>
                <input
                    type="range"
                    min={-50}
                    max={50}
                    step={0.1}
                    value={tempoPct}
                    onChange={(e) => handleTempoChange(parseFloat(e.target.value))}
                    style={{ flex: 1, accentColor: accentColor, height: 3, cursor: "pointer" }}
                />
                <span className="mono" style={{ fontSize: 9, minWidth: 46, color: "var(--text-secondary)", textAlign: "right" }}>
                    {tempoPct >= 0 ? "+" : ""}{tempoPct.toFixed(1)}%
                </span>
                <button className="btn btn-ghost btn-icon" style={{ width: 16, height: 16 }} title="Reset tempo" onClick={() => handleTempoChange(0)}>↺</button>
            </div>

        </div>
    );
}
