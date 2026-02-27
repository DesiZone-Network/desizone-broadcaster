import { useEffect, useState } from "react";
import { MessageSquare, Check, X, Clock, Music2, ListPlus } from "lucide-react";
import {
    addToQueue,
    acceptRequestP3,
    getRequestHistoryLog,
    getPendingRequests,
    getSong,
    rejectRequestP3,
    RequestLogEntry,
    SamSong,
} from "../../lib/bridge";
import { writeEventLog } from "../../lib/bridge7";
import { serializeSongDragPayload } from "../../lib/songDrag";

interface RequestRow {
    request: RequestLogEntry;
    song: SamSong | null;
}

function timeAgo(epochSecs: number): string {
    const diff = Math.max(0, Math.floor(Date.now() / 1000 - epochSecs));
    if (diff < 60) return `${diff}s ago`;
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
    const [pendingRows, setPendingRows] = useState<RequestRow[]>([]);
    const [historyRows, setHistoryRows] = useState<RequestRow[]>([]);
    const [loading, setLoading] = useState(false);
    const [busyRequestId, setBusyRequestId] = useState<number | null>(null);

    const loadRequests = async () => {
        setLoading(true);
        try {
            const pending = await getPendingRequests();
            const all = await getRequestHistoryLog(500, 0);

            const hydrateRows = async (rows: RequestLogEntry[]): Promise<RequestRow[]> => {
                const uniqueSongIds = Array.from(
                    new Set(rows.map((r) => r.song_id).filter((songId) => songId > 0))
                );
                const songs = await Promise.all(
                    uniqueSongIds.map(async (songId) => ({
                        songId,
                        song: await getSong(songId).catch(() => null),
                    }))
                );
                const songMap = new Map<number, SamSong | null>(
                    songs.map((entry) => [entry.songId, entry.song])
                );
                return rows.map((request) => ({
                    request,
                    song: songMap.get(request.song_id) ?? null,
                }));
            };

            const [hydratedPending, hydratedAll] = await Promise.all([
                hydrateRows(pending),
                hydrateRows(all),
            ]);

            setPendingRows(hydratedPending);
            setHistoryRows(hydratedAll);
        } catch (e) {
            console.error(e);
            setPendingRows([]);
            setHistoryRows([]);
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
        setBusyRequestId(id);
        await acceptRequestP3(id).catch(console.error);
        setBusyRequestId(null);
        await loadRequests();
    };

    const handleReject = async (id: number | null) => {
        if (id == null) return;
        setBusyRequestId(id);
        await rejectRequestP3(id).catch(console.error);
        setBusyRequestId(null);
        await loadRequests();
    };

    const handlePromoteToQueue = async (row: RequestRow) => {
        const requestId = row.request.id;
        if (requestId == null) return;
        try {
            setBusyRequestId(requestId);
            await addToQueue(row.request.song_id);
            await acceptRequestP3(requestId);
            await writeEventLog({
                level: "info",
                category: "scheduler",
                event: "request_promoted_to_queue",
                message: `Promoted request ${requestId} to queue (song_id=${row.request.song_id})`,
                songId: row.request.song_id,
            });
        } catch (e) {
            console.error(e);
        } finally {
            setBusyRequestId(null);
        }
        await loadRequests();
    };

    const statusStyles: Record<RequestLogEntry["status"], { label: string; color: string; border: string; bg: string }> = {
        pending: {
            label: "Pending",
            color: "var(--amber)",
            border: "var(--amber-dim)",
            bg: "var(--amber-glow)",
        },
        accepted: {
            label: "Accepted",
            color: "var(--green)",
            border: "var(--green-dim)",
            bg: "var(--green-glow)",
        },
        rejected: {
            label: "Rejected",
            color: "var(--red)",
            border: "var(--red-dim)",
            bg: "var(--red-glow)",
        },
        played: {
            label: "Played",
            color: "var(--cyan)",
            border: "rgba(6,182,212,.45)",
            bg: "rgba(6,182,212,.12)",
        },
    };

    return (
        <div className="flex flex-col h-full">
            <div className="flex items-center justify-between" style={{ padding: "6px 12px", borderBottom: "1px solid var(--border-default)", flexShrink: 0 }}>
                <div className="flex items-center gap-2">
                    <MessageSquare size={12} style={{ color: "var(--text-muted)" }} />
                    <span className="section-label">Requests</span>
                    {pendingRows.length > 0 && (
                        <span className="mono" style={{ fontSize: 10, fontWeight: 700, color: "#fff", background: "var(--red)", padding: "1px 6px", borderRadius: 10 }}>
                            {pendingRows.length}
                        </span>
                    )}
                </div>
            </div>

            <div className="overflow-auto flex-1" style={{ padding: "6px 8px" }}>
                {loading && pendingRows.length === 0 && historyRows.length === 0 ? (
                    <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                        <MessageSquare size={20} />
                        <span style={{ fontSize: 11 }}>Loading requests…</span>
                    </div>
                ) : (
                    <div className="flex flex-col gap-8">
                        <div>
                            <div
                                className="flex items-center justify-between"
                                style={{ marginBottom: 6, padding: "0 4px" }}
                            >
                                <span className="section-label">Active Pending</span>
                                <span className="mono text-muted" style={{ fontSize: 10 }}>
                                    {pendingRows.length}
                                </span>
                            </div>
                            {pendingRows.length === 0 ? (
                                <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                                    <MessageSquare size={20} />
                                    <span style={{ fontSize: 11 }}>No pending requests</span>
                                </div>
                            ) : (
                                pendingRows.map((row) => {
                                    const req = row.request;
                                    const song = row.song;
                                    const isBusy = busyRequestId != null && busyRequestId === req.id;
                                    return (
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
                                                <button
                                                    className="btn btn-ghost"
                                                    style={{ fontSize: 10, padding: "3px 10px", color: "var(--cyan)", borderColor: "rgba(6,182,212,.45)", background: "rgba(6,182,212,.12)" }}
                                                    disabled={isBusy}
                                                    onClick={() => handlePromoteToQueue(row)}
                                                >
                                                    <ListPlus size={11} />Add to Queue
                                                </button>
                                                <button
                                                    className="btn btn-ghost"
                                                    style={{ fontSize: 10, padding: "3px 10px", color: "var(--green)", borderColor: "var(--green-dim)", background: "var(--green-glow)" }}
                                                    disabled={isBusy}
                                                    onClick={() => handleAccept(req.id)}
                                                >
                                                    <Check size={11} />Accept
                                                </button>
                                                <button
                                                    className="btn btn-ghost"
                                                    style={{ fontSize: 10, padding: "3px 10px", color: "var(--red)", borderColor: "var(--red-dim)", background: "var(--red-glow)" }}
                                                    disabled={isBusy}
                                                    onClick={() => handleReject(req.id)}
                                                >
                                                    <X size={11} />Reject
                                                </button>
                                            </div>
                                        </div>
                                    );
                                })
                            )}
                        </div>

                        <div>
                            <div
                                className="flex items-center justify-between"
                                style={{ marginBottom: 6, padding: "0 4px" }}
                            >
                                <span className="section-label">All Requests</span>
                                <span className="mono text-muted" style={{ fontSize: 10 }}>
                                    {historyRows.length}
                                </span>
                            </div>
                            {historyRows.length === 0 ? (
                                <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                                    <MessageSquare size={20} />
                                    <span style={{ fontSize: 11 }}>No request history yet</span>
                                </div>
                            ) : (
                                historyRows.map(({ request: req, song }) => {
                                    const status = statusStyles[req.status];
                                    return (
                                        <div
                                            key={`history-${req.id ?? `${req.song_id}-${req.requested_at}`}`}
                                            className="list-row"
                                            style={{ alignItems: "center", padding: "7px 10px", gap: 8 }}
                                        >
                                            <span
                                                className="mono"
                                                style={{
                                                    fontSize: 9,
                                                    fontWeight: 700,
                                                    letterSpacing: "0.06em",
                                                    borderRadius: 10,
                                                    padding: "2px 6px",
                                                    color: status.color,
                                                    border: `1px solid ${status.border}`,
                                                    background: status.bg,
                                                    flexShrink: 0,
                                                }}
                                            >
                                                {status.label}
                                            </span>
                                            <div style={{ minWidth: 0, flex: 1 }}>
                                                <div
                                                    className="font-medium"
                                                    style={{
                                                        fontSize: 12,
                                                        color: "var(--text-primary)",
                                                        overflow: "hidden",
                                                        textOverflow: "ellipsis",
                                                        whiteSpace: "nowrap",
                                                    }}
                                                >
                                                    {song?.title || req.song_title || `Song #${req.song_id}`}
                                                </div>
                                                <div
                                                    className="text-muted"
                                                    style={{
                                                        fontSize: 10,
                                                        overflow: "hidden",
                                                        textOverflow: "ellipsis",
                                                        whiteSpace: "nowrap",
                                                    }}
                                                >
                                                    {(song?.artist || req.artist || "Unknown Artist")} · {req.requester_name || "Listener"}
                                                </div>
                                            </div>
                                            <div className="flex items-center gap-1 text-muted" style={{ flexShrink: 0 }}>
                                                <Clock size={9} />
                                                <span style={{ fontSize: 10 }}>{timeAgo(req.requested_at)}</span>
                                            </div>
                                        </div>
                                    );
                                })
                            )}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
