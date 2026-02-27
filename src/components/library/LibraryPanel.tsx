import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import type { ReactNode } from "react";
import {
    Search, Music2, Plus, Folder, FolderOpen, RefreshCw, Database,
    ChevronDown, ChevronRight, Settings2, Pencil, X, Save,
} from "lucide-react";
import {
    searchSongs,
    getSamCategories,
    getSongsInCategory,
    getSamDbStatus,
    createSamCategory,
    addToQueue,
    getSongsByWeightRange,
    getSongTypes,
    updateSong,
    loadTrack,
    onDeckStateChanged,
} from "../../lib/bridge";
import type { SamSong, SamCategory } from "../../lib/bridge";
import { resolveAlbumArtUrl } from "../../lib/albumArt";
import { serializeSongDragPayload } from "../../lib/songDrag";

// ── Constants ──────────────────────────────────────────────────────────────

const SONGTYPE_LABELS: Record<string, string> = {
    S: "Normal Song",
    I: "Station ID",
    P: "Promo",
    J: "Jingle",
    A: "Advertisement",
    N: "Syndicated News",
    V: "Interviews",
    X: "Sound FX",
    C: "Unknown Content",
    O: "Other",
    D: "Demo",
};

const ROTATION_BANDS = [
    { label: "Power Hit",       min: 90,  max: 101, color: "#f59e0b", icon: "⭐" },
    { label: "Heavy Rotation",  min: 80,  max: 90,  color: "#ef4444", icon: "🔥" },
    { label: "Medium Rotation", min: 60,  max: 80,  color: "#3b82f6", icon: "📡" },
    { label: "Light Rotation",  min: 40,  max: 60,  color: "#22c55e", icon: "💿" },
    { label: "Rare Rotation",   min: 20,  max: 40,  color: "#a3a3a3", icon: "🔄" },
    { label: "No Rotation",     min: 0,   max: 20,  color: "#6b7280", icon: "⏸" },
] as const;

// ── Types ──────────────────────────────────────────────────────────────────

type LibraryFilter =
    | { kind: "all" }
    | { kind: "songtype"; value: string }
    | { kind: "rotation"; min: number; max: number; label: string }
    | { kind: "category"; id: number; name: string };

interface SearchOpts {
    artist: boolean;
    title: boolean;
    album: boolean;
    filename: boolean;
}

interface CategoryRow {
    category: SamCategory;
    depth: number;
}

interface PersistedLibraryState {
    filter?: LibraryFilter;
    query?: string;
    searchOpts?: SearchOpts;
    typeFilter?: string;
    typesCollapsed?: boolean;
    rotationCollapsed?: boolean;
    foldersCollapsed?: boolean;
}

const LIBRARY_STATE_KEY = "desizone.library.panel.v1";

function readPersistedLibraryState(): PersistedLibraryState {
    try {
        const raw = window.localStorage.getItem(LIBRARY_STATE_KEY);
        if (!raw) return {};
        const parsed = JSON.parse(raw) as PersistedLibraryState;
        return parsed && typeof parsed === "object" ? parsed : {};
    } catch {
        return {};
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

function formatDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
}

function matchQuery(song: SamSong, q: string, opts: SearchOpts): boolean {
    if (!q.trim()) return true;
    const lq = q.toLowerCase();
    // If all unchecked, default to artist + title
    const any = opts.artist || opts.title || opts.album || opts.filename;
    if (!any || opts.artist) {
        if (song.artist.toLowerCase().includes(lq)) return true;
    }
    if (!any || opts.title) {
        if ((song.title || "").toLowerCase().includes(lq)) return true;
    }
    if (opts.album) {
        if ((song.album || "").toLowerCase().includes(lq)) return true;
    }
    if (opts.filename) {
        if (song.filename.toLowerCase().includes(lq)) return true;
    }
    return false;
}

function byCategoryOrder(a: SamCategory, b: SamCategory): number {
    if (a.itemindex !== b.itemindex) return a.itemindex - b.itemindex;
    if (a.levelindex !== b.levelindex) return a.levelindex - b.levelindex;
    return a.catname.localeCompare(b.catname);
}

function extractSpotifyTrackId(value: string): string | null {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const direct = trimmed.match(/^[A-Za-z0-9]{22}$/);
    if (direct) return direct[0];
    const trackUrl = trimmed.match(/track\/([A-Za-z0-9]{22})/);
    if (trackUrl) return trackUrl[1];
    return null;
}

function parseXfadeOverride(raw: string): Record<string, number> {
    const out: Record<string, number> = {};
    const params = new URLSearchParams(raw.startsWith("&") ? raw.slice(1) : raw);
    for (const [k, v] of params.entries()) {
        const n = Number(v);
        if (Number.isFinite(n)) out[k] = n;
    }
    return out;
}

function buildCategoryRows(categories: SamCategory[]): CategoryRow[] {
    if (categories.length === 0) return [];

    const allIds = new Set(categories.map((c) => c.id));
    const childrenByParent = new Map<number, SamCategory[]>();

    for (const category of categories) {
        // If parent is missing, treat as top-level.
        const parent = allIds.has(category.parent_id) ? category.parent_id : 0;
        const bucket = childrenByParent.get(parent) ?? [];
        bucket.push(category);
        childrenByParent.set(parent, bucket);
    }

    for (const siblings of childrenByParent.values()) {
        siblings.sort(byCategoryOrder);
    }

    const rows: CategoryRow[] = [];
    const visited = new Set<number>();

    const walk = (parentId: number, depth: number) => {
        const children = childrenByParent.get(parentId) ?? [];
        for (const child of children) {
            if (visited.has(child.id)) continue;
            visited.add(child.id);
            rows.push({ category: child, depth });
            walk(child.id, depth + 1);
        }
    };

    walk(0, 0);

    // Defensive fallback for malformed/cyclic parent data.
    if (rows.length < categories.length) {
        const remaining = categories.filter((c) => !visited.has(c.id)).sort(byCategoryOrder);
        for (const category of remaining) rows.push({ category, depth: 0 });
    }

    return rows;
}

// ── SongRow ────────────────────────────────────────────────────────────────

function SongRow({
    song,
    selected,
    onSelect,
    onAddToQueue,
    onEdit,
    onLoadToDeckA,
    onLoadToDeckB,
    onSmartLoad,
}: {
    song: SamSong;
    selected: boolean;
    onSelect: () => void;
    onAddToQueue: () => void;
    onEdit: (song: SamSong) => void;
    onLoadToDeckA: () => void;
    onLoadToDeckB: () => void;
    onSmartLoad: () => void;
}) {
    return (
        <div
            className={`list-row ${selected ? "selected" : ""}`}
            onClick={onSelect}
            onDoubleClick={onSmartLoad}
            style={{ cursor: "default" }}
            draggable
            onDragStart={(e) => {
                // Use text/plain — WebKit (Tauri macOS) silently drops
                // custom MIME types like "application/desizone-song"
                e.dataTransfer.setData("text/plain", serializeSongDragPayload(song, "library"));
                e.dataTransfer.effectAllowed = "copy";
            }}
        >
            <Music2
                size={11}
                style={{ color: selected ? "var(--amber)" : "var(--text-muted)", flexShrink: 0 }}
            />

            <div className="min-w-0" style={{ flex: 2 }}>
                <div
                    style={{
                        fontSize: 12,
                        fontWeight: 500,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                        color: selected ? "var(--amber)" : "var(--text-primary)",
                    }}
                >
                    {song.title || song.filename.split(/[\\/]/).pop()}
                </div>
            </div>

            <div className="min-w-0" style={{ flex: 1.5 }}>
                <div
                    className="text-muted"
                    style={{
                        fontSize: 11,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                    }}
                >
                    {song.artist || "Unknown Artist"}
                </div>
            </div>

            <span
                className="mono text-muted"
                style={{ fontSize: 10, minWidth: 34, textAlign: "right" }}
            >
                {song.bpm > 0 ? song.bpm : "--"}
            </span>

            <span
                className="mono text-muted"
                style={{ fontSize: 10, minWidth: 36, textAlign: "right" }}
            >
                {formatDuration(song.duration)}
            </span>

            {/* Load to Deck A */}
            <button
                className="btn btn-ghost btn-icon"
                style={{
                    width: 18,
                    height: 18,
                    marginLeft: 1,
                    fontSize: 9,
                    fontWeight: 700,
                    color: "var(--amber)",
                    opacity: 0.55,
                    transition: "opacity 0.15s",
                    letterSpacing: 0,
                }}
                onClick={(e) => { e.stopPropagation(); onLoadToDeckA(); }}
                title="Load to Deck A"
            >
                A
            </button>

            {/* Load to Deck B */}
            <button
                className="btn btn-ghost btn-icon"
                style={{
                    width: 18,
                    height: 18,
                    marginLeft: 1,
                    fontSize: 9,
                    fontWeight: 700,
                    color: "var(--cyan)",
                    opacity: 0.55,
                    transition: "opacity 0.15s",
                    letterSpacing: 0,
                }}
                onClick={(e) => { e.stopPropagation(); onLoadToDeckB(); }}
                title="Load to Deck B"
            >
                B
            </button>

            {/* Edit button */}
            <button
                className="btn btn-ghost btn-icon"
                style={{
                    width: 22,
                    height: 22,
                    marginLeft: 2,
                    opacity: selected ? 1 : 0.25,
                    transition: "opacity 0.15s",
                }}
                onClick={(e) => { e.stopPropagation(); onEdit(song); }}
                title="Edit song info"
            >
                <Pencil size={11} />
            </button>

            {/* Add to queue button */}
            <button
                className="btn btn-ghost btn-icon"
                style={{
                    width: 22,
                    height: 22,
                    marginLeft: 2,
                    opacity: selected ? 1 : 0.35,
                    transition: "opacity 0.15s",
                }}
                onClick={(e) => { e.stopPropagation(); onAddToQueue(); }}
                title="Add to queue"
            >
                <Plus size={12} />
            </button>
        </div>
    );
}

// ── EditSongDialog ─────────────────────────────────────────────────────────

const SONGTYPE_OPTIONS = [
    { value: "S", label: "S — Normal Song" },
    { value: "I", label: "I — Station ID" },
    { value: "P", label: "P — Promo" },
    { value: "J", label: "J — Jingle" },
    { value: "A", label: "A — Advertisement" },
    { value: "N", label: "N — Syndicated News" },
    { value: "V", label: "V — Interviews" },
    { value: "X", label: "X — Sound FX" },
    { value: "C", label: "C — Unknown Content" },
    { value: "O", label: "O — Other" },
    { value: "D", label: "D — Demo" },
];

function FieldRow({ label, children }: { label: string; children: ReactNode }) {
    return (
        <div style={{ display: "grid", gridTemplateColumns: "100px 1fr", alignItems: "center", gap: 8, marginBottom: 8 }}>
            <label style={{ fontSize: 11, color: "var(--text-muted)", textAlign: "right" }}>{label}</label>
            {children}
        </div>
    );
}

function EditSongDialog({
    song,
    onClose,
    onSaved,
}: {
    song: SamSong;
    onClose: () => void;
    onSaved: (updated: Partial<SamSong>) => void;
}) {
    const [saving, setSaving] = useState(false);
    const [saveError, setSaveError] = useState<string | null>(null);
    const [albumArtLoadFailed, setAlbumArtLoadFailed] = useState(false);

    // Editable fields mirroring SongUpdateFields
    const [artist,    setArtist]    = useState(song.artist ?? "");
    const [title,     setTitle]     = useState(song.title ?? "");
    const [album,     setAlbum]     = useState(song.album ?? "");
    const [genre,     setGenre]     = useState(song.genre ?? "");
    const [albumyear, setAlbumyear] = useState(song.albumyear ?? "");
    const [songtype,  setSongtype]  = useState(song.songtype ?? "S");
    const [weight,    setWeight]    = useState(String(song.weight ?? 50));
    const [bpm,       setBpm]       = useState(String(song.bpm ?? ""));
    const [mood,      setMood]      = useState(song.mood ?? "");
    const [rating,    setRating]    = useState(String(song.rating ?? ""));
    const [label,     setLabel]     = useState(song.label ?? "");
    const [isrc,      setIsrc]      = useState(song.isrc ?? "");
    const [spotifyId, setSpotifyId] = useState(song.upc ?? "");
    const [xfade,     setXfade]     = useState(song.xfade ?? "");
    const [overlay,   setOverlay]   = useState(song.overlay === "yes" ? "yes" : "no");

    // Field style helper
    const inp: React.CSSProperties = {
        width: "100%",
        padding: "4px 8px",
        fontSize: 12,
        background: "var(--bg-input)",
        border: "1px solid var(--border-strong)",
        borderRadius: "var(--r-md)",
        color: "var(--text-primary)",
        outline: "none",
    };

    const spotifyTrackId = extractSpotifyTrackId(spotifyId);
    const parsedXfade = parseXfadeOverride(xfade);
    const albumArtSrc = resolveAlbumArtUrl(song.picture);

    useEffect(() => {
        setAlbumArtLoadFailed(false);
    }, [song.id, song.picture]);

    const handleSave = async () => {
        setSaving(true);
        setSaveError(null);
        try {
            const fields: Record<string, string | number | null> = {};
            if (artist    !== (song.artist    ?? "")) fields.artist    = artist;
            if (title     !== (song.title     ?? "")) fields.title     = title;
            if (album     !== (song.album     ?? "")) fields.album     = album;
            if (genre     !== (song.genre     ?? "")) fields.genre     = genre;
            if (albumyear !== (song.albumyear ?? "")) fields.albumyear = albumyear;
            if (songtype  !== (song.songtype  ?? "S"))fields.songtype  = songtype;
            if (mood      !== (song.mood      ?? "")) fields.mood      = mood;
            if (label     !== (song.label     ?? "")) fields.label     = label;
            if (isrc      !== (song.isrc      ?? "")) fields.isrc      = isrc;
            if (spotifyId !== (song.upc       ?? "")) fields.upc       = spotifyId;
            if (xfade     !== (song.xfade     ?? "")) fields.xfade     = xfade;
            // Numeric
            const wNum = parseFloat(weight);
            if (!isNaN(wNum) && wNum !== (song.weight ?? 50)) fields.weight = wNum;
            const bpmNum = parseInt(bpm, 10);
            if (!isNaN(bpmNum) && bpmNum !== (song.bpm ?? 0)) fields.bpm = bpmNum;
            const ratNum = parseInt(rating, 10);
            if (!isNaN(ratNum) && ratNum !== (song.rating ?? 0)) fields.rating = ratNum;
            if (overlay !== (song.overlay ?? "no")) fields.overlay = overlay;

            if (Object.keys(fields).length === 0) { onClose(); return; }

            await updateSong(song.id, fields as any);
            onSaved(fields as Partial<SamSong>);
            onClose();
        } catch (e) {
            setSaveError(e instanceof Error ? e.message : String(e));
        } finally {
            setSaving(false);
        }
    };

    // Close on backdrop click
    const handleBackdrop = (e: React.MouseEvent<HTMLDivElement>) => {
        if (e.target === e.currentTarget) onClose();
    };

    return (
        <div
            onClick={handleBackdrop}
            style={{
                position: "fixed", inset: 0, zIndex: 9999,
                background: "rgba(0,0,0,0.65)",
                display: "flex", alignItems: "center", justifyContent: "center",
            }}
        >
            <div
                style={{
                    background: "var(--bg-panel)",
                    border: "1px solid var(--border-strong)",
                    borderRadius: "var(--r-lg)",
                    width: 540,
                    maxHeight: "88vh",
                    overflow: "auto",
                    padding: "18px 22px 16px",
                    boxShadow: "0 12px 48px rgba(0,0,0,0.6)",
                }}
            >
                {/* Header */}
                <div className="flex items-center justify-between" style={{ marginBottom: 16 }}>
                    <div>
                        <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                            Edit Song Info
                        </span>
                        <div style={{ fontSize: 10, color: "var(--text-muted)", marginTop: 2 }}>
                            ID #{song.id} · {song.filename.split(/[\\/]/).pop()}
                        </div>
                    </div>
                    <button
                        className="btn btn-ghost btn-icon"
                        onClick={onClose}
                        style={{ width: 24, height: 24 }}
                    >
                        <X size={13} />
                    </button>
                </div>

                {/* Fields */}
                <FieldRow label="Album Art">
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                        {albumArtSrc && !albumArtLoadFailed ? (
                            <img
                                src={albumArtSrc}
                                alt="Album art"
                                onError={() => setAlbumArtLoadFailed(true)}
                                style={{ width: 72, height: 72, objectFit: "cover", borderRadius: 6, border: "1px solid var(--border-strong)" }}
                            />
                        ) : (
                            <div
                                style={{
                                    width: 72,
                                    height: 72,
                                    borderRadius: 6,
                                    border: "1px solid var(--border-strong)",
                                    background: "var(--bg-input)",
                                    color: "var(--text-muted)",
                                    display: "flex",
                                    alignItems: "center",
                                    justifyContent: "center",
                                    fontSize: 10,
                                    textAlign: "center",
                                    padding: 6,
                                }}
                            >
                                No Album Art
                            </div>
                        )}
                        <div className="mono" style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>
                            {song.picture || "(empty)"}
                        </div>
                    </div>
                </FieldRow>
                <FieldRow label="Title">
                    <input style={inp} value={title} onChange={e => setTitle(e.target.value)} />
                </FieldRow>
                <FieldRow label="Artist">
                    <input style={inp} value={artist} onChange={e => setArtist(e.target.value)} />
                </FieldRow>
                <FieldRow label="Album">
                    <input style={inp} value={album} onChange={e => setAlbum(e.target.value)} />
                </FieldRow>
                <FieldRow label="Genre">
                    <input style={inp} value={genre} onChange={e => setGenre(e.target.value)} />
                </FieldRow>
                <FieldRow label="Year">
                    <input style={inp} value={albumyear} onChange={e => setAlbumyear(e.target.value)} maxLength={4} />
                </FieldRow>
                <FieldRow label="Song Type">
                    <select
                        style={{ ...inp, cursor: "pointer" }}
                        value={songtype}
                        onChange={e => setSongtype(e.target.value)}
                    >
                        {SONGTYPE_OPTIONS.map(o => (
                            <option key={o.value} value={o.value}>{o.label}</option>
                        ))}
                    </select>
                </FieldRow>

                <div style={{ borderTop: "1px solid var(--border-default)", margin: "10px 0" }} />

                <FieldRow label="Weight">
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <input
                            style={{ ...inp, width: 70 }}
                            type="number" min={0} max={100} step={1}
                            value={weight}
                            onChange={e => setWeight(e.target.value)}
                        />
                        <input
                            type="range" min={0} max={100} step={1}
                            value={parseFloat(weight) || 0}
                            onChange={e => setWeight(e.target.value)}
                            style={{ flex: 1, accentColor: "var(--amber)" }}
                        />
                        <span className="mono" style={{ fontSize: 10, color: "var(--amber)", minWidth: 28 }}>
                            {weight}
                        </span>
                    </div>
                </FieldRow>
                <FieldRow label="BPM">
                    <input style={{ ...inp, width: 80 }} type="number" min={0} max={300} value={bpm} onChange={e => setBpm(e.target.value)} />
                </FieldRow>
                <FieldRow label="Rating">
                    <input style={{ ...inp, width: 80 }} type="number" min={0} max={5} value={rating} onChange={e => setRating(e.target.value)} />
                </FieldRow>
                <FieldRow label="Overlay">
                    <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
                        <input
                            type="checkbox"
                            checked={overlay === "yes"}
                            onChange={e => setOverlay(e.target.checked ? "yes" : "no")}
                            style={{ accentColor: "var(--amber)", width: 14, height: 14 }}
                        />
                        <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                            Allow overlapping play
                        </span>
                    </label>
                </FieldRow>

                <div style={{ borderTop: "1px solid var(--border-default)", margin: "10px 0" }} />

                <FieldRow label="Mood">
                    <input style={inp} value={mood} onChange={e => setMood(e.target.value)} />
                </FieldRow>
                <FieldRow label="Label">
                    <input style={inp} value={label} onChange={e => setLabel(e.target.value)} />
                </FieldRow>
                <FieldRow label="ISRC">
                    <input style={inp} value={isrc} onChange={e => setIsrc(e.target.value)} maxLength={12} />
                </FieldRow>
                <FieldRow label="Spotify ID">
                    <div>
                        <input style={inp} value={spotifyId} onChange={e => setSpotifyId(e.target.value)} placeholder="Track ID or Spotify track URL" />
                        {spotifyTrackId && (
                            <iframe
                                data-testid="embed-iframe"
                                style={{ borderRadius: 12, marginTop: 8, width: "100%" }}
                                src={`https://open.spotify.com/embed/track/${spotifyTrackId}?utm_source=generator`}
                                width="100%"
                                height="152"
                                frameBorder="0"
                                allow="autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture"
                                loading="lazy"
                                title="Spotify track preview"
                            />
                        )}
                    </div>
                </FieldRow>
                <FieldRow label="Per-song Fade">
                    <div>
                        <input style={inp} value={xfade} onChange={e => setXfade(e.target.value)} placeholder="&fie=0&foe=0&xf=2&ge=0&gl=29" />
                        <div style={{ fontSize: 10, color: "var(--text-muted)", marginTop: 6 }}>
                            Parsed: {Object.keys(parsedXfade).length === 0
                                ? "No key/value fade overrides found"
                                : Object.entries(parsedXfade).map(([k, v]) => `${k}=${v}`).join(" · ")}
                        </div>
                    </div>
                </FieldRow>

                {/* Filename (read-only) */}
                <div style={{ borderTop: "1px solid var(--border-default)", margin: "10px 0" }} />
                <FieldRow label="File">
                    <div
                        className="mono"
                        style={{
                            fontSize: 10, color: "var(--text-muted)",
                            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                        }}
                        title={song.filename}
                    >
                        {song.filename}
                    </div>
                </FieldRow>

                {/* Error */}
                {saveError && (
                    <div style={{ fontSize: 11, color: "#ef4444", marginBottom: 10, textAlign: "center" }}>
                        ⚠ {saveError}
                    </div>
                )}

                {/* Footer buttons */}
                <div className="flex items-center justify-end gap-2" style={{ marginTop: 14 }}>
                    <button
                        className="btn btn-ghost"
                        style={{ padding: "5px 14px", fontSize: 12 }}
                        onClick={onClose}
                        disabled={saving}
                    >
                        Cancel
                    </button>
                    <button
                        className="btn btn-primary"
                        style={{
                            padding: "5px 16px", fontSize: 12,
                            background: "var(--amber)", color: "#000", border: "none",
                            borderRadius: "var(--r-md)", fontWeight: 600, cursor: "pointer",
                            display: "flex", alignItems: "center", gap: 6,
                            opacity: saving ? 0.6 : 1,
                        }}
                        onClick={handleSave}
                        disabled={saving}
                    >
                        <Save size={12} />
                        {saving ? "Saving…" : "Save Changes"}
                    </button>
                </div>
            </div>
        </div>
    );
}

// ── SidebarItem ─────────────────────────────────────────────────────────────

function SidebarItem({
    active,
    icon,
    label,
    color,
    indentLevel = 0,
    onClick,
}: {
    active: boolean;
    icon: ReactNode;
    label: string;
    color?: string;
    indentLevel?: number;
    onClick: () => void;
}) {
    return (
        <div
            className={`list-row ${active ? "selected" : ""}`}
            onClick={onClick}
            style={{
                cursor: "default",
                padding: "3px 8px",
                gap: 5,
                paddingLeft: 8 + Math.min(indentLevel, 8) * 10,
            }}
        >
            <span
                style={{
                    flexShrink: 0,
                    display: "flex",
                    alignItems: "center",
                    color: active ? "var(--amber)" : (color ?? "var(--cyan)"),
                }}
            >
                {icon}
            </span>
            <span
                style={{
                    fontSize: 11,
                    flex: 1,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    color: active ? "var(--amber)" : "var(--text-primary)",
                }}
                title={label}
            >
                {label}
            </span>
        </div>
    );
}

// ── SectionLabel ────────────────────────────────────────────────────────────

function SectionLabel({
    label,
    collapsed,
    onToggle,
}: {
    label: string;
    collapsed: boolean;
    onToggle: () => void;
}) {
    return (
        <div
            style={{
                display: "flex",
                alignItems: "center",
                gap: 4,
                padding: "7px 6px 2px 6px",
                cursor: "pointer",
                userSelect: "none",
            }}
            onClick={onToggle}
        >
            {collapsed
                ? <ChevronRight size={10} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                : <ChevronDown  size={10} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
            }
            <span className="section-label" style={{ letterSpacing: "0.04em" }}>
                {label}
            </span>
        </div>
    );
}

// ── LibraryPanel ──────────────────────────────────────────────────────────

export function LibraryPanel() {
    const [persistedState] = useState<PersistedLibraryState>(() => readPersistedLibraryState());
    const [connected, setConnected]         = useState(false);

    // sidebar data
    const [categories, setCategories]       = useState<SamCategory[]>([]);
    const [songTypes,  setSongTypes]        = useState<string[]>([]);
    const categoryRows = useMemo(() => buildCategoryRows(categories), [categories]);

    // sidebar section collapse
    const [typesCollapsed,    setTypesCollapsed]    = useState(!!persistedState.typesCollapsed);
    const [rotationCollapsed, setRotationCollapsed] = useState(!!persistedState.rotationCollapsed);
    const [foldersCollapsed,  setFoldersCollapsed]  = useState(!!persistedState.foldersCollapsed);

    // active filter (discriminated union)
    const [filter, setFilter] = useState<LibraryFilter>(() => {
        const candidate = persistedState.filter;
        if (!candidate || typeof candidate !== "object") return { kind: "all" };
        switch (candidate.kind) {
            case "all":
                return { kind: "all" };
            case "songtype":
                return typeof candidate.value === "string"
                    ? { kind: "songtype", value: candidate.value }
                    : { kind: "all" };
            case "rotation":
                return (typeof candidate.min === "number" &&
                    typeof candidate.max === "number" &&
                    typeof candidate.label === "string")
                    ? {
                        kind: "rotation",
                        min: candidate.min,
                        max: candidate.max,
                        label: candidate.label,
                    }
                    : { kind: "all" };
            case "category":
                return (typeof candidate.id === "number" && typeof candidate.name === "string")
                    ? { kind: "category", id: candidate.id, name: candidate.name }
                    : { kind: "all" };
            default:
                return { kind: "all" };
        }
    });

    // song list
    const [songs,    setSongs]   = useState<SamSong[]>([]);
    const [loading,  setLoading] = useState(false);
    const [error,    setError]   = useState<string | null>(null);
    const [selected, setSelected] = useState<number | null>(null);

    // song edit dialog
    const [editSong, setEditSong] = useState<SamSong | null>(null);

    // search
    const [query, setQuery]         = useState(persistedState.query ?? "");
    const [showOpts, setShowOpts]   = useState(false);
    const [searchOpts, setSearchOpts] = useState<SearchOpts>(() => ({
        artist: persistedState.searchOpts?.artist ?? true,
        title: persistedState.searchOpts?.title ?? true,
        album: persistedState.searchOpts?.album ?? false,
        filename: persistedState.searchOpts?.filename ?? false,
    }));
    // type filter from advanced options dropdown (only applies when filter.kind === "all")
    const [typeFilter, setTypeFilter] = useState<string>(persistedState.typeFilter ?? "");

    const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

    // ── Deck state (for smart double-click load) ───────────────────────────
    // Track deck_a idle so double-click can route to the free deck
    const [deckAIdle, setDeckAIdle] = useState(true);
    const [loadMsg, setLoadMsg] = useState<string | null>(null);
    const [creatingFolder, setCreatingFolder] = useState(false);
    const loadMsgTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
        const unsub = onDeckStateChanged((e) => {
            if (e.deck === "deck_a") setDeckAIdle(e.state === "idle");
        });
        return () => { unsub.then((f) => f()); };
    }, []);

    const showLoadMsg = (msg: string, isError = false) => {
        setLoadMsg(isError ? `⚠ ${msg}` : msg);
        if (loadMsgTimer.current) clearTimeout(loadMsgTimer.current);
        loadMsgTimer.current = setTimeout(() => setLoadMsg(null), isError ? 5000 : 2500);
    };

    const handleLoadToDeck = useCallback(async (deckId: "deck_a" | "deck_b", song: SamSong) => {
        try {
            await loadTrack(deckId, song.filename, song.id);
            showLoadMsg(`Loaded "${song.title || song.filename.split(/[\\/]/).pop()}" → ${deckId === "deck_a" ? "Deck A" : "Deck B"}`);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            showLoadMsg(msg, true);
        }
    }, []);

    const handleSmartLoad = useCallback(async (song: SamSong) => {
        // Load to Deck A if it's idle; fall back to Deck B
        const target = deckAIdle ? "deck_a" : "deck_b";
        await handleLoadToDeck(target, song);
    }, [deckAIdle, handleLoadToDeck]);

    // ── Poll SAM DB connection ─────────────────────────────────────────────

    const checkConnection = useCallback(async () => {
        try {
            const status = await getSamDbStatus();
            setConnected(status.connected);
        } catch {
            setConnected(false);
        }
    }, []);

    useEffect(() => {
        checkConnection();
        const id = setInterval(checkConnection, 3000);
        return () => clearInterval(id);
    }, [checkConnection]);

    // ── Load sidebar data when DB comes online ────────────────────────────

    useEffect(() => {
        if (!connected) {
            setCategories([]);
            setSongTypes([]);
            setSongs([]);
            return;
        }
        getSamCategories()
            .then(setCategories)
            .catch(() => setCategories([]));
        getSongTypes()
            .then(setSongTypes)
            .catch(() => setSongTypes([]));
    }, [connected]);

    useEffect(() => {
        // If a previously saved category no longer exists, fall back cleanly.
        if (filter.kind === "category" && categories.length > 0) {
            const exists = categories.some((cat) => cat.id === filter.id);
            if (!exists) setFilter({ kind: "all" });
        }
    }, [filter, categories]);

    useEffect(() => {
        try {
            const stateToPersist: PersistedLibraryState = {
                filter,
                query,
                searchOpts,
                typeFilter,
                typesCollapsed,
                rotationCollapsed,
                foldersCollapsed,
            };
            window.localStorage.setItem(LIBRARY_STATE_KEY, JSON.stringify(stateToPersist));
        } catch {
            // Ignore storage quota/privacy errors.
        }
    }, [
        filter,
        query,
        searchOpts,
        typeFilter,
        typesCollapsed,
        rotationCollapsed,
        foldersCollapsed,
    ]);

    // ── Unified fetchSongs ────────────────────────────────────────────────

    const fetchSongs = useCallback(
        async (f: LibraryFilter, q: string, opts: SearchOpts, typeF: string) => {
            setLoading(true);
            setError(null);
            try {
                let result: SamSong[];

                if (f.kind === "all" || f.kind === "songtype") {
                    const songType: string | undefined =
                        f.kind === "songtype" ? f.value :
                        typeF                ? typeF    : undefined;
                    result = await searchSongs(q, 500, 0, {
                        searchArtist:   opts.artist,
                        searchTitle:    opts.title,
                        searchAlbum:    opts.album,
                        searchFilename: opts.filename,
                        songType,
                    });
                } else if (f.kind === "rotation") {
                    const all = await getSongsByWeightRange(f.min, f.max, 500);
                    result = q.trim() ? all.filter((s) => matchQuery(s, q, opts)) : all;
                } else {
                    // category
                    const all = await getSongsInCategory(f.id, 500);
                    result = q.trim() ? all.filter((s) => matchQuery(s, q, opts)) : all;
                }
                setSongs(result);
            } catch (e: unknown) {
                setError(e instanceof Error ? e.message : String(e));
                setSongs([]);
            } finally {
                setLoading(false);
            }
        },
        []
    );

    // Debounce re-fetch on any dependency change
    useEffect(() => {
        if (!connected) return;
        clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(
            () => fetchSongs(filter, query, searchOpts, typeFilter),
            300
        );
        return () => clearTimeout(debounceRef.current);
    }, [connected, filter, query, searchOpts, typeFilter, fetchSongs]);

    // ── Actions ───────────────────────────────────────────────────────────

    const handleAddToQueue = async (songId: number) => {
        try {
            await addToQueue(songId);
        } catch (e) {
            console.error("addToQueue error:", e);
        }
    };

    const handleCreateFolder = async () => {
        if (creatingFolder) return;
        const name = window.prompt("New folder name");
        if (!name || !name.trim()) return;

        const parentId = filter.kind === "category" ? filter.id : 0;
        setCreatingFolder(true);
        try {
            const created = await createSamCategory(name.trim(), parentId);
            const updated = await getSamCategories();
            setCategories(updated);
            applyFilter({ kind: "category", id: created.id, name: created.catname });
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            setError(msg);
        } finally {
            setCreatingFolder(false);
        }
    };

    const applyFilter = (f: LibraryFilter) => {
        setFilter(f);
        setSelected(null);
        // preserve search query so the user can search within a new filter
    };

    // ── Helper: is a given filter the active one? ─────────────────────────

    const isActive = (f: LibraryFilter): boolean => {
        if (filter.kind !== f.kind) return false;
        switch (f.kind) {
            case "all":       return true;
            case "songtype":  return filter.kind === "songtype"  && filter.value === f.value;
            case "rotation":  return filter.kind === "rotation"  && filter.min   === f.min;
            case "category":  return filter.kind === "category"  && filter.id    === f.id;
        }
    };

    // ── Not-connected placeholder ─────────────────────────────────────────

    if (!connected) {
        return (
            <div
                className="flex flex-col items-center justify-center gap-3 h-full"
                style={{ color: "var(--text-muted)" }}
            >
                <Database size={28} style={{ opacity: 0.4 }} />
                <span style={{ fontSize: 12 }}>SAM DB not connected</span>
                <span style={{ fontSize: 10, opacity: 0.6 }}>
                    Open Settings → SAM Database to connect
                </span>
            </div>
        );
    }

    // Label used in search placeholder + footer
    const filterLabel =
        filter.kind === "all"       ? "All Songs"
        : filter.kind === "songtype" ? (SONGTYPE_LABELS[filter.value] ?? `Type ${filter.value}`)
        : filter.kind === "rotation" ? filter.label
        : filter.name;

    // ── Layout ────────────────────────────────────────────────────────────

    return (
        <div className="flex h-full" style={{ overflow: "hidden" }}>

            {/* ── Sidebar ──────────────────────────────────────────────────── */}
            <div
                style={{
                    width: 162,
                    minWidth: 162,
                    flexShrink: 0,
                    borderRight: "1px solid var(--border-default)",
                    display: "flex",
                    flexDirection: "column",
                    overflow: "hidden",
                }}
            >
                <div className="overflow-auto flex-1" style={{ paddingBottom: 8 }}>

                    {/* All Songs */}
                    <div style={{ padding: "4px 0 2px 0" }}>
                        <SidebarItem
                            active={isActive({ kind: "all" })}
                            icon={isActive({ kind: "all" })
                                ? <FolderOpen size={12} />
                                : <Folder     size={12} />
                            }
                            label="All Songs"
                            onClick={() => applyFilter({ kind: "all" })}
                        />
                    </div>

                    {/* ── Song Types ── */}
                    {songTypes.length > 0 && (
                        <>
                            <SectionLabel
                                label="Song Types"
                                collapsed={typesCollapsed}
                                onToggle={() => setTypesCollapsed((v) => !v)}
                            />
                            {!typesCollapsed && songTypes.map((t) => (
                                <SidebarItem
                                    key={t}
                                    active={isActive({ kind: "songtype", value: t })}
                                    icon={<Music2 size={11} />}
                                    label={SONGTYPE_LABELS[t] ?? `Type: ${t}`}
                                    color="var(--text-muted)"
                                    onClick={() => applyFilter({ kind: "songtype", value: t })}
                                />
                            ))}
                        </>
                    )}

                    {/* ── Rotation ── */}
                    <SectionLabel
                        label="Rotation"
                        collapsed={rotationCollapsed}
                        onToggle={() => setRotationCollapsed((v) => !v)}
                    />
                    {!rotationCollapsed && ROTATION_BANDS.map((band) => (
                        <SidebarItem
                            key={band.label}
                            active={isActive({ kind: "rotation", min: band.min, max: band.max, label: band.label })}
                            icon={<span style={{ fontSize: 12, lineHeight: 1 }}>{band.icon}</span>}
                            label={band.label}
                            color={band.color}
                            onClick={() =>
                                applyFilter({ kind: "rotation", min: band.min, max: band.max, label: band.label })
                            }
                        />
                    ))}

                    {/* ── Folders ── */}
                    <div className="flex items-center justify-between" style={{ paddingRight: 6 }}>
                        <SectionLabel
                            label="Folders"
                            collapsed={foldersCollapsed}
                            onToggle={() => setFoldersCollapsed((v) => !v)}
                        />
                        <button
                            className="btn btn-ghost btn-icon"
                            style={{ width: 18, height: 18, marginTop: 5, opacity: creatingFolder ? 0.5 : 0.9 }}
                            onClick={handleCreateFolder}
                            title="Create folder"
                            disabled={creatingFolder}
                        >
                            <Plus size={11} />
                        </button>
                    </div>
                    {!foldersCollapsed && categoryRows.map(({ category: cat, depth }) => (
                        <SidebarItem
                            key={cat.id}
                            active={isActive({ kind: "category", id: cat.id, name: cat.catname })}
                            icon={isActive({ kind: "category", id: cat.id, name: cat.catname })
                                ? <FolderOpen size={12} />
                                : <Folder     size={12} />
                            }
                            label={cat.catname}
                            indentLevel={depth}
                            onClick={() =>
                                applyFilter({ kind: "category", id: cat.id, name: cat.catname })
                            }
                        />
                    ))}
                    {!foldersCollapsed && categories.length === 0 && (
                        <div className="text-muted" style={{ padding: "4px 10px", fontSize: 10 }}>
                            No folders in DB
                        </div>
                    )}
                </div>
            </div>

            {/* ── Song list pane ───────────────────────────────────────────── */}
            <div className="flex flex-col flex-1 min-w-0" style={{ overflow: "hidden" }}>

                {/* Search bar row */}
                <div
                    style={{
                        padding: "6px 8px 4px 8px",
                        borderBottom: "1px solid var(--border-default)",
                        flexShrink: 0,
                    }}
                >
                    <div className="flex items-center gap-2">
                        <div style={{ position: "relative", flex: 1 }}>
                            <Search
                                size={12}
                                style={{
                                    position: "absolute", left: 8, top: "50%",
                                    transform: "translateY(-50%)",
                                    color: "var(--text-muted)", pointerEvents: "none",
                                }}
                            />
                            <input
                                type="search"
                                className="input"
                                placeholder={`Search in ${filterLabel}…`}
                                value={query}
                                onChange={(e) => setQuery(e.target.value)}
                                style={{ paddingLeft: 26, fontSize: 11 }}
                            />
                        </div>
                        <button
                            className="btn btn-ghost btn-icon"
                            style={{
                                width: 26,
                                height: 26,
                                flexShrink: 0,
                                opacity: showOpts ? 1 : 0.5,
                                color: showOpts ? "var(--amber)" : undefined,
                            }}
                            onClick={() => setShowOpts((v) => !v)}
                            title="Search options"
                        >
                            <Settings2 size={13} />
                        </button>
                    </div>

                    {/* Advanced options row */}
                    {showOpts && (
                        <div
                            className="flex items-center flex-wrap gap-x-3 gap-y-1"
                            style={{
                                marginTop: 5,
                                paddingTop: 4,
                                borderTop: "1px solid var(--border-subtle)",
                            }}
                        >
                            {(
                                [
                                    { key: "artist",   label: "Artist"   },
                                    { key: "title",    label: "Title"    },
                                    { key: "album",    label: "Album"    },
                                    { key: "filename", label: "Filename" },
                                ] as { key: keyof SearchOpts; label: string }[]
                            ).map(({ key, label }) => (
                                <label
                                    key={key}
                                    className="flex items-center gap-1"
                                    style={{ cursor: "pointer", userSelect: "none" }}
                                >
                                    <input
                                        type="checkbox"
                                        checked={searchOpts[key]}
                                        onChange={(e) =>
                                            setSearchOpts((o) => ({ ...o, [key]: e.target.checked }))
                                        }
                                        style={{ accentColor: "var(--amber)", width: 11, height: 11 }}
                                    />
                                    <span style={{ fontSize: 10, color: "var(--text-secondary)" }}>
                                        {label}
                                    </span>
                                </label>
                            ))}

                            {/* Type filter dropdown — only meaningful on "All Songs" */}
                            <div style={{ marginLeft: "auto" }}>
                                <select
                                    className="input"
                                    style={{ fontSize: 10, height: 22, padding: "0 4px" }}
                                    value={typeFilter}
                                    onChange={(e) => setTypeFilter(e.target.value)}
                                    title="Filter by song type"
                                >
                                    <option value="">All Types</option>
                                    {songTypes.map((t) => (
                                        <option key={t} value={t}>
                                            {t} — {SONGTYPE_LABELS[t] ?? t}
                                        </option>
                                    ))}
                                </select>
                            </div>
                        </div>
                    )}
                </div>

                {/* Column headers */}
                <div
                    className="flex items-center"
                    style={{
                        padding: "3px 8px",
                        borderBottom: "1px solid var(--border-subtle)",
                        background: "var(--bg-surface)",
                        flexShrink: 0,
                        gap: 6,
                    }}
                >
                    <span style={{ width: 11 }} />
                    <span className="section-label" style={{ flex: 2 }}>Title</span>
                    <span className="section-label" style={{ flex: 1.5 }}>Artist</span>
                    <span className="section-label" style={{ minWidth: 34, textAlign: "right" }}>BPM</span>
                    <span className="section-label" style={{ minWidth: 36, textAlign: "right" }}>Dur</span>
                    <span style={{ width: 26 }} />
                </div>

                {/* Song rows */}
                <div className="overflow-auto flex-1" style={{ padding: "4px 6px" }}>
                    {loading ? (
                        <div
                            className="flex items-center justify-center gap-2"
                            style={{ height: 80, color: "var(--text-muted)", fontSize: 12 }}
                        >
                            <RefreshCw size={13} style={{ animation: "spin 1s linear infinite" }} />
                            Loading…
                        </div>
                    ) : error ? (
                        <div
                            className="flex flex-col items-center justify-center gap-1"
                            style={{ height: 80, fontSize: 11, color: "var(--red, #f87171)" }}
                        >
                            <span>Failed to load songs</span>
                            <span style={{ fontSize: 10, opacity: 0.7 }}>{error}</span>
                        </div>
                    ) : songs.length === 0 ? (
                        <div
                            className="flex flex-col items-center justify-center gap-2"
                            style={{ height: 80, color: "var(--text-muted)" }}
                        >
                            <Music2 size={20} style={{ opacity: 0.4 }} />
                            <span style={{ fontSize: 11 }}>
                                {query ? "No results for that search" : "No songs in this folder"}
                            </span>
                        </div>
                    ) : (
                        songs.map((song) => (
                            <SongRow
                                key={song.id}
                                song={song}
                                selected={selected === song.id}
                                onSelect={() => setSelected(song.id)}
                                onAddToQueue={() => handleAddToQueue(song.id)}
                                onEdit={(s) => setEditSong(s)}
                                onLoadToDeckA={() => handleLoadToDeck("deck_a", song)}
                                onLoadToDeckB={() => handleLoadToDeck("deck_b", song)}
                                onSmartLoad={() => handleSmartLoad(song)}
                            />
                        ))
                    )}
                </div>

                {/* Footer */}
                <div
                    style={{
                        padding: "3px 10px",
                        borderTop: "1px solid var(--border-default)",
                        background: "var(--bg-surface)",
                        flexShrink: 0,
                    }}
                >
                    {loadMsg ? (
                        <span style={{
                            fontSize: 10,
                            color: loadMsg.startsWith("⚠") ? "#ef4444" : "var(--cyan)",
                            fontWeight: 500,
                        }}>
                            {loadMsg}
                        </span>
                    ) : (
                        <span className="text-muted" style={{ fontSize: 10 }}>
                            {loading
                                ? "Loading…"
                                : songs.length > 0
                                    ? `${songs.length} track${songs.length === 1 ? "" : "s"} · ${filterLabel} · dbl-click or A/B buttons to load to deck`
                                    : "No tracks in this folder"}
                        </span>
                    )}
                </div>
            </div>

            {/* Song edit dialog — rendered as a portal-like overlay */}
            {editSong && (
                <EditSongDialog
                    song={editSong}
                    onClose={() => setEditSong(null)}
                    onSaved={(updated) => {
                        // Patch the in-memory song list so the row reflects changes immediately
                        setSongs((prev) =>
                            prev.map((s) =>
                                s.id === editSong.id ? { ...s, ...updated } : s
                            )
                        );
                    }}
                />
            )}
        </div>
    );
}
