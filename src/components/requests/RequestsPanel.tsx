import { useEffect, useState } from "react";
import { MessageSquare, Check, X, Clock, Music2 } from "lucide-react";
import {
    acceptRequestP3,
    getPendingRequests,
    getSong,
    rejectRequestP3,
    RequestLogEntry,
    SamSong,
} from "../../lib/bridge";
import { serializeSongDragPayload } from "../../lib/songDrag";

interface RequestRow {
    request: RequestLogEntry;
    song: SamSong | null;
}

function timeAgo(epochSecs: number): string {
    const diff = Math.floor(Date.now() / 1000 - epochSecs);
    if (diff < 60) return `${Math.max(0, diff)}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    return `${Math.floor(diff / 3600)}h ago`;
}

function PlatformBadge({ platform }: { platform: string | null }) {
    const key = (platform || "web").toLowerCase();
    const configs: Record<string, { label: string; color: string }> = {
        web: { label: "WEB", color: "var(--cyan)" },
        discord: { label: "DC", color: "#5865F2" },
        app: { label: "APP", color: "var(--amber)" },
    };
    const c = configs[key] ?? { label: key.slice(0, 3).toUpperCase(), color: "var(--text-muted)" };
    return (
        <span
            style={{
                fontSize: 8,
                fontWeight: 700,
                letterSpacing: "0.1em",
                padding: "1px 5px",
                borderRadius: 10,
                background: `${c.color}20`,
                border: `1px solid ${c.color}60`,
                color: c.color,
            }}
        >
            {c.label}
        </span>
    );
}

export function RequestsPanel() {
    const [rows, setRows] = useState<RequestRow[]>([]);
    const [loading, setLoading] = useState(false);

    const loadRequests = async () => {
        setLoading(true);
        try {
            const pending = await getPendingRequests();
            const hydrated = await Promise.all(
                pending.map(async (r) => ({
                    request: r,
                    song: r.song_id ? await getSong(r.song_id).catch(() => null) : null,
                }))
            );
            setRows(hydrated);
        } catch (e) {
            console.error(e);
            setRows([]);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadRequests();
        const id = setInterval(loadRequests, 6000);
        return () => clearInterval(id);
    }, []);

    const handleAccept = async (id: number | null) => {
        if (id == null) return;
        await acceptRequestP3(id).catch(console.error);
        await loadRequests();
    };

    const handleReject = async (id: number | null) => {
        if (id == null) return;
        await rejectRequestP3(id).catch(console.error);
        await loadRequests();
    };

    return (
        <div className="flex flex-col h-full">
            <div className="flex items-center justify-between" style={{ padding: "6px 12px", borderBottom: "1px solid var(--border-default)", flexShrink: 0 }}>
                <div className="flex items-center gap-2">
                    <MessageSquare size={12} style={{ color: "var(--text-muted)" }} />
                    <span className="section-label">Requests</span>
                    {rows.length > 0 && (
                        <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: "#fff", background: "var(--red)", padding: "1px 6px", borderRadius: 10 }}>
                            {rows.length}
                        </span>
                    )}
                </div>
            </div>

            <div className="overflow-auto flex-1" style={{ padding: "6px 8px" }}>
                {loading && rows.length === 0 ? (
                    <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                        <MessageSquare size={20} />
                        <span style={{ fontSize: 11 }}>Loading requests…</span>
                    </div>
                ) : rows.length === 0 ? (
                    <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                        <MessageSquare size={20} />
                        <span style={{ fontSize: 11 }}>No pending requests</span>
                    </div>
                ) : (
                    rows.map(({ request: req, song }) => (
                        <div
                            key={req.id ?? `${req.song_id}-${req.requested_at}`}
                            className="list-row"
                            style={{ alignItems: "flex-start", padding: "8px 10px", flexDirection: "column", gap: 6 }}
                            draggable={Boolean(song?.filename)}
                            onDragStart={(e) => {
                                if (!song?.filename) return;
                                e.dataTransfer.setData("text/plain", serializeSongDragPayload(song, "requests"));
                                e.dataTransfer.effectAllowed = "copy";
                            }}
                        >
                            <div className="flex items-center justify-between w-full">
                                <div className="flex items-center gap-2">
                                    <PlatformBadge platform={req.requester_platform} />
                                    <span style={{ fontSize: 11, fontWeight: 500 }}>{req.requester_name || "Listener"}</span>
                                </div>
                                <div className="flex items-center gap-1 text-muted">
                                    <Clock size={9} />
                                    <span style={{ fontSize: 10 }}>{timeAgo(req.requested_at)}</span>
                                </div>
                            </div>

                            <div className="flex items-center gap-2">
                                <Music2 size={11} style={{ color: "var(--text-muted)" }} />
                                <div>
                                    <div className="font-medium" style={{ fontSize: 12, color: "var(--text-primary)" }}>
                                        {song?.title || req.song_title || `Song #${req.song_id}`}
                                    </div>
                                    <div className="text-muted" style={{ fontSize: 10 }}>{song?.artist || req.artist || "Unknown Artist"}</div>
                                </div>
                            </div>

                            <div className="flex gap-2" style={{ marginTop: 2 }}>
                                <button className="btn btn-ghost" style={{ fontSize: 10, padding: "3px 10px", color: "var(--green)", borderColor: "var(--green-dim)", background: "var(--green-glow)" }} onClick={() => handleAccept(req.id)}>
                                    <Check size={11} />Accept
                                </button>
                                <button className="btn btn-ghost" style={{ fontSize: 10, padding: "3px 10px", color: "var(--red)", borderColor: "var(--red-dim)", background: "var(--red-glow)" }} onClick={() => handleReject(req.id)}>
                                    <X size={11} />Reject
                                </button>
                            </div>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
