import { useState, useEffect, useCallback, useRef } from "react";
import { Search, Music2, Plus } from "lucide-react";
import { searchSongs, addToQueue, SamSong } from "../../lib/bridge";

export function LibraryPanel() {
    const [query, setQuery] = useState("");
    const [results, setResults] = useState<SamSong[]>([]);
    const [loading, setLoading] = useState(false);
    const [selected, setSelected] = useState<number | null>(null);
    const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

    const doSearch = useCallback(async (q: string) => {
        setLoading(true);
        try {
            const r = await searchSongs(q, 100);
            setResults(r);
        } catch (e) {
            console.error(e);
            setResults([]);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => doSearch(query), 300);
        return () => clearTimeout(debounceRef.current);
    }, [query, doSearch]);

    const handleAddToQueue = async (songId: number) => {
        try {
            await addToQueue(songId);
        } catch (e) {
            console.error(e);
        }
    };

    function formatDuration(secs: number): string {
        const m = Math.floor(secs / 60);
        const s = secs % 60;
        return `${m}:${s.toString().padStart(2, "0")}`;
    }

    return (
        <div className="flex flex-col h-full">
            {/* Search header */}
            <div style={{ padding: "8px 10px", borderBottom: "1px solid var(--border-default)", flexShrink: 0 }}>
                <div style={{ position: "relative" }}>
                    <Search
                        size={13}
                        style={{
                            position: "absolute", left: 10, top: "50%", transform: "translateY(-50%)",
                            color: "var(--text-muted)", pointerEvents: "none",
                        }}
                    />
                    <input
                        type="search"
                        className="input"
                        placeholder="Search artist, title, album…"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        style={{ paddingLeft: 30, fontSize: 12 }}
                    />
                </div>
            </div>

            {/* Column headers */}
            <div
                className="flex items-center"
                style={{
                    padding: "4px 10px",
                    borderBottom: "1px solid var(--border-subtle)",
                    background: "var(--bg-surface)",
                    flexShrink: 0,
                }}
            >
                {[
                    { label: "Title", flex: 2 },
                    { label: "Artist", flex: 1.5 },
                    { label: "Dur", flex: 0 },
                    { label: "", flex: 0 },
                ].map((col) => (
                    <span
                        key={col.label}
                        className="section-label"
                        style={{ flex: col.flex || "none", minWidth: col.flex === 0 ? 40 : undefined }}
                    >
                        {col.label}
                    </span>
                ))}
            </div>

            {/* Results */}
            <div className="overflow-auto flex-1" style={{ padding: "4px 6px" }}>
                {loading ? (
                    <div className="flex items-center justify-center" style={{ height: 80, color: "var(--text-muted)", fontSize: 12 }}>
                        Searching…
                    </div>
                ) : results.length === 0 ? (
                    <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                        <Music2 size={20} />
                        <span style={{ fontSize: 11 }}>{query ? "No results found" : "Search the media library above"}</span>
                    </div>
                ) : (
                    results.map((song) => (
                        <div
                            key={song.songid}
                            className={`list-row ${selected === song.songid ? "selected" : ""}`}
                            onClick={() => setSelected(song.songid)}
                            onDoubleClick={() => handleAddToQueue(song.songid)}
                        >
                            <Music2 size={11} style={{ color: "var(--text-muted)", flexShrink: 0 }} />

                            <div className="min-w-0" style={{ flex: 2 }}>
                                <div
                                    style={{
                                        fontSize: 12,
                                        fontWeight: 500,
                                        overflow: "hidden",
                                        textOverflow: "ellipsis",
                                        whiteSpace: "nowrap",
                                        color: selected === song.songid ? "var(--amber)" : "var(--text-primary)",
                                    }}
                                >
                                    {song.title}
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
                                    {song.artist}
                                </div>
                            </div>

                            <span className="mono text-muted" style={{ fontSize: 10, minWidth: 36, textAlign: "right" }}>
                                {formatDuration(song.duration)}
                            </span>

                            <button
                                className="btn btn-ghost btn-icon"
                                style={{ width: 22, height: 22, marginLeft: 4, opacity: selected === song.songid ? 1 : 0 }}
                                onClick={(e) => { e.stopPropagation(); handleAddToQueue(song.songid); }}
                                title="Add to queue"
                            >
                                <Plus size={12} />
                            </button>
                        </div>
                    ))
                )}
            </div>

            {/* Footer */}
            <div
                style={{
                    padding: "4px 10px",
                    borderTop: "1px solid var(--border-default)",
                    background: "var(--bg-surface)",
                    flexShrink: 0,
                }}
            >
                <span className="text-muted" style={{ fontSize: 10 }}>
                    {results.length > 0 ? `${results.length} tracks` : "No tracks"} — double-click to add to queue
                </span>
            </div>
        </div>
    );
}
