import { useState, useEffect, useCallback } from "react";
import { List, Music2, MessageSquare, History, Terminal } from "lucide-react";
import { QueuePanel } from "../queue/QueuePanel";
import { LibraryPanel } from "../library/LibraryPanel";
import { RequestsPanel } from "../requests/RequestsPanel";
import { getHistory, onDeckStateChanged } from "../../lib/bridge";
import type { HistoryEntry } from "../../lib/bridge";
import { getEventLog } from "../../lib/bridge7";
import type { EventLogEntry } from "../../lib/bridge7";

type TabId = "queue" | "library" | "requests" | "history" | "logs";

const TABS: { id: TabId; label: string; icon: React.ReactNode; badge?: number }[] = [
    { id: "queue", label: "Queue", icon: <List size={12} /> },
    { id: "library", label: "Library", icon: <Music2 size={12} /> },
    { id: "requests", label: "Requests", icon: <MessageSquare size={12} />, badge: 2 },
    { id: "history", label: "History", icon: <History size={12} /> },
    { id: "logs", label: "Logs", icon: <Terminal size={12} /> },
];

function fmtTime(ts: number | string): string {
    let d: Date;
    if (typeof ts === "string") {
        // SAM/MySQL often returns "YYYY-MM-DD HH:mm:ss"; normalize for WebKit.
        const normalized = ts.includes("T") ? ts : ts.replace(" ", "T");
        d = new Date(normalized);
        if (Number.isNaN(d.getTime())) {
            const m = ts.match(
                /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2}):(\d{2})$/,
            );
            if (m) {
                d = new Date(
                    Number(m[1]),
                    Number(m[2]) - 1,
                    Number(m[3]),
                    Number(m[4]),
                    Number(m[5]),
                    Number(m[6]),
                );
            }
        }
    } else {
        d = new Date(ts * 1000);
    }
    if (Number.isNaN(d.getTime())) return "--:--:--";
    return d.toLocaleTimeString("en-GB", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
    });
}

function fmtDuration(secs: number): string {
    const s = Math.max(0, Math.round(secs));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export function BottomPanel() {
    const [activeTab, setActiveTab] = useState<TabId>("queue");
    const [history, setHistory] = useState<HistoryEntry[]>([]);
    const [historyLoading, setHistoryLoading] = useState(false);
    const [historyError, setHistoryError] = useState<string | null>(null);
    const [logs, setLogs] = useState<EventLogEntry[]>([]);
    const [logsLoading, setLogsLoading] = useState(false);

    const loadHistory = useCallback((silent = false) => {
        if (!silent) {
            setHistoryLoading(true);
        }
        if (!silent) {
            setHistoryError(null);
        }
        getHistory(100)
            .then(setHistory)
            .catch((e) => setHistoryError(String(e)))
            .finally(() => {
                if (!silent) {
                    setHistoryLoading(false);
                }
            });
    }, []);

    useEffect(() => {
        if (activeTab !== "history") {
            return;
        }
        loadHistory(false);
        const intervalId = setInterval(() => loadHistory(true), 3000);
        const unsub = onDeckStateChanged((e) => {
            if ((e.deck === "deck_a" || e.deck === "deck_b") && e.state === "idle") {
                loadHistory(true);
            }
        });
        return () => {
            clearInterval(intervalId);
            unsub.then((f) => f());
        };
    }, [activeTab, loadHistory]);

    useEffect(() => {
        if (activeTab === "logs") {
            setLogsLoading(true);
            getEventLog({ limit: 200, offset: 0 })
                .then((res) => setLogs(res.events))
                .catch(() => { })
                .finally(() => setLogsLoading(false));
        }
    }, [activeTab]);

    return (
        <div
            className="flex flex-col"
            style={{
                background: "var(--bg-panel)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--r-lg)",
                flex: 1,
                minHeight: 0,
                overflow: "hidden",
            }}
        >
            {/* Tab bar */}
            <div className="tabs-list" style={{ paddingLeft: 0, paddingRight: 0 }}>
                {TABS.map((tab) => (
                    <button
                        key={tab.id}
                        className="tab-trigger"
                        data-state={activeTab === tab.id ? "active" : "inactive"}
                        onClick={() => setActiveTab(tab.id)}
                        style={{ display: "flex", alignItems: "center", gap: 6 }}
                    >
                        {tab.icon}
                        {tab.label}
                        {tab.badge != null && tab.id !== activeTab && (
                            <span
                                style={{
                                    fontSize: 9, fontWeight: 700,
                                    background: "var(--red)", color: "#fff",
                                    borderRadius: 10, padding: "0px 5px",
                                    lineHeight: "14px",
                                }}
                            >
                                {tab.badge}
                            </span>
                        )}
                    </button>
                ))}
            </div>

            {/* Tab content */}
            <div className="flex-1 min-h-0 overflow-hidden">
                {activeTab === "queue" && <QueuePanel />}
                {activeTab === "library" && <LibraryPanel />}
                {activeTab === "requests" && <RequestsPanel />}
                {activeTab === "history" && (
                    <div style={{ height: "100%", overflow: "auto" }}>
                        {historyLoading && (
                            <div style={{ padding: "20px 16px", fontSize: 11, color: "var(--text-muted)" }}>
                                Loading history…
                            </div>
                        )}
                        {historyError && (
                            <div style={{ padding: "20px 16px", fontSize: 11, color: "var(--red)" }}>
                                {historyError.includes("not connected") || historyError.includes("SAM")
                                    ? "SAM Database not connected — open Settings to configure."
                                    : historyError}
                            </div>
                        )}
                        {!historyLoading && !historyError && history.length === 0 && (
                            <div
                                className="flex flex-col items-center justify-center"
                                style={{ height: "100%", color: "var(--text-muted)", fontSize: 12 }}
                            >
                                <History size={24} style={{ marginBottom: 8, opacity: 0.4 }} />
                                No history yet
                            </div>
                        )}
                        {!historyLoading && history.length > 0 && (
                            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 11 }}>
                                <thead>
                                    <tr style={{ borderBottom: "1px solid var(--border-default)", position: "sticky", top: 0, background: "var(--bg-panel)" }}>
                                        <th style={{ padding: "5px 12px", textAlign: "left", color: "var(--text-dim)", fontWeight: 500 }}>Time</th>
                                        <th style={{ padding: "5px 12px", textAlign: "left", color: "var(--text-dim)", fontWeight: 500 }}>Artist</th>
                                        <th style={{ padding: "5px 12px", textAlign: "left", color: "var(--text-dim)", fontWeight: 500 }}>Title</th>
                                        <th style={{ padding: "5px 8px", textAlign: "right", color: "var(--text-dim)", fontWeight: 500 }}>Dur</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {history.map((h) => (
                                        <tr
                                            key={h.id}
                                            style={{ borderBottom: "1px solid var(--border-default)" }}
                                        >
                                            <td style={{ padding: "4px 12px", color: "var(--text-dim)", fontFamily: "var(--font-mono)", whiteSpace: "nowrap" }}>
                                                {fmtTime(h.date_played)}
                                            </td>
                                            <td style={{ padding: "4px 12px", color: "var(--text-muted)", overflow: "hidden", maxWidth: 160, textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                                {h.artist}
                                            </td>
                                            <td style={{ padding: "4px 12px", color: "var(--text-primary)", overflow: "hidden", maxWidth: 220, textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                                {h.title}
                                            </td>
                                            <td style={{ padding: "4px 8px", color: "var(--text-dim)", textAlign: "right", fontFamily: "var(--font-mono)" }}>
                                                {fmtDuration(h.duration)}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </div>
                )}
                {activeTab === "logs" && (
                    <div
                        style={{
                            height: "100%",
                            padding: "8px 12px",
                            overflow: "auto",
                            fontFamily: "var(--font-mono)",
                            fontSize: 11,
                            color: "var(--text-muted)",
                        }}
                    >
                        {logsLoading && <div style={{ color: "var(--text-dim)" }}>Loading…</div>}
                        {!logsLoading && logs.length === 0 && (
                            <div style={{ color: "var(--text-dim)" }}>No events logged yet.</div>
                        )}
                        {logs.map((e) => {
                            const color =
                                e.level === "error" ? "var(--red)"
                                : e.level === "warn" ? "var(--amber)"
                                : e.level === "info" ? "var(--cyan)"
                                : "var(--text-muted)";
                            return (
                                <div key={e.id} style={{ color }}>
                                    [{fmtTime(e.timestamp)}] [{e.category}] {e.message}
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>
        </div>
    );
}
