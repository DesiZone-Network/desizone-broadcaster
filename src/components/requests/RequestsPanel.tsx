import { useState } from "react";
import { MessageSquare, Check, X, Clock } from "lucide-react";

// Placeholder types until backend is wired
interface Request {
    id: number;
    songTitle: string;
    artist: string;
    requesterName: string;
    requesterPlatform: "web" | "discord" | "app";
    requestedAt: Date;
    status: "pending" | "accepted" | "rejected";
}

// Mock data for display — replace with invoke('get_requests') once live
const MOCK_REQUESTS: Request[] = [
    {
        id: 1,
        songTitle: "Dil Se Re",
        artist: "A.R. Rahman",
        requesterName: "Aisha K.",
        requesterPlatform: "web",
        requestedAt: new Date(Date.now() - 3 * 60000),
        status: "pending",
    },
    {
        id: 2,
        songTitle: "Ek Ladki Ko Dekha",
        artist: "RD Burman",
        requesterName: "Ravi S.",
        requesterPlatform: "discord",
        requestedAt: new Date(Date.now() - 8 * 60000),
        status: "pending",
    },
];

function timeAgo(date: Date): string {
    const diff = Math.floor((Date.now() - date.getTime()) / 1000);
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    return `${Math.floor(diff / 3600)}h ago`;
}

function PlatformBadge({ platform }: { platform: Request["requesterPlatform"] }) {
    const configs = {
        web: { label: "WEB", color: "var(--cyan)" },
        discord: { label: "DC", color: "#5865F2" },
        app: { label: "APP", color: "var(--amber)" },
    };
    const c = configs[platform];
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
    const [requests, setRequests] = useState<Request[]>(MOCK_REQUESTS);

    const handleAccept = (id: number) => {
        setRequests((prev) =>
            prev.map((r) => (r.id === id ? { ...r, status: "accepted" } : r))
        );
        // TODO: invoke('accept_request', { requestId: id })
    };

    const handleReject = (id: number) => {
        setRequests((prev) =>
            prev.map((r) => (r.id === id ? { ...r, status: "rejected" } : r))
        );
        // TODO: invoke('reject_request', { requestId: id })
    };

    const pending = requests.filter((r) => r.status === "pending");

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div
                className="flex items-center justify-between"
                style={{ padding: "6px 12px", borderBottom: "1px solid var(--border-default)", flexShrink: 0 }}
            >
                <div className="flex items-center gap-2">
                    <MessageSquare size={12} style={{ color: "var(--text-muted)" }} />
                    <span className="section-label">Requests</span>
                    {pending.length > 0 && (
                        <span
                            className="mono"
                            style={{
                                fontSize: 10, fontWeight: 700,
                                color: "#fff", background: "var(--red)",
                                padding: "1px 6px", borderRadius: 10,
                            }}
                        >
                            {pending.length}
                        </span>
                    )}
                </div>
            </div>

            {/* List */}
            <div className="overflow-auto flex-1" style={{ padding: "6px 8px" }}>
                {requests.length === 0 ? (
                    <div className="flex flex-col items-center justify-center gap-2" style={{ height: 80, color: "var(--text-muted)" }}>
                        <MessageSquare size={20} />
                        <span style={{ fontSize: 11 }}>No requests</span>
                    </div>
                ) : (
                    requests.map((req) => (
                        <div
                            key={req.id}
                            className="list-row"
                            style={{
                                opacity: req.status !== "pending" ? 0.5 : 1,
                                alignItems: "flex-start",
                                padding: "8px 10px",
                                flexDirection: "column",
                                gap: 6,
                                background: req.status === "accepted"
                                    ? "var(--green-glow)"
                                    : req.status === "rejected"
                                        ? "var(--red-glow)"
                                        : undefined,
                                borderColor: req.status === "accepted"
                                    ? "var(--green-dim)"
                                    : req.status === "rejected"
                                        ? "var(--red-dim)"
                                        : undefined,
                            }}
                        >
                            <div className="flex items-center justify-between w-full">
                                <div className="flex items-center gap-2">
                                    <PlatformBadge platform={req.requesterPlatform} />
                                    <span style={{ fontSize: 11, fontWeight: 500 }}>{req.requesterName}</span>
                                </div>
                                <div className="flex items-center gap-1 text-muted">
                                    <Clock size={9} />
                                    <span style={{ fontSize: 10 }}>{timeAgo(req.requestedAt)}</span>
                                </div>
                            </div>

                            <div>
                                <div className="font-medium" style={{ fontSize: 12, color: "var(--text-primary)" }}>
                                    {req.songTitle}
                                </div>
                                <div className="text-muted" style={{ fontSize: 10 }}>{req.artist}</div>
                            </div>

                            {req.status === "pending" && (
                                <div className="flex gap-2" style={{ marginTop: 2 }}>
                                    <button
                                        className="btn btn-ghost"
                                        style={{
                                            fontSize: 10, padding: "3px 10px",
                                            color: "var(--green)", borderColor: "var(--green-dim)",
                                            background: "var(--green-glow)",
                                        }}
                                        onClick={() => handleAccept(req.id)}
                                    >
                                        <Check size={11} />
                                        Accept
                                    </button>
                                    <button
                                        className="btn btn-ghost"
                                        style={{
                                            fontSize: 10, padding: "3px 10px",
                                            color: "var(--red)", borderColor: "var(--red-dim)",
                                            background: "var(--red-glow)",
                                        }}
                                        onClick={() => handleReject(req.id)}
                                    >
                                        <X size={11} />
                                        Reject
                                    </button>
                                </div>
                            )}
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
