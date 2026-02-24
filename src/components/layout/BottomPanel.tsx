import { useState } from "react";
import { List, Music2, MessageSquare, History, Terminal } from "lucide-react";
import { QueuePanel } from "../queue/QueuePanel";
import { LibraryPanel } from "../library/LibraryPanel";
import { RequestsPanel } from "../requests/RequestsPanel";

type TabId = "queue" | "library" | "requests" | "history" | "logs";

const TABS: { id: TabId; label: string; icon: React.ReactNode; badge?: number }[] = [
    { id: "queue", label: "Queue", icon: <List size={12} /> },
    { id: "library", label: "Library", icon: <Music2 size={12} /> },
    { id: "requests", label: "Requests", icon: <MessageSquare size={12} />, badge: 2 },
    { id: "history", label: "History", icon: <History size={12} /> },
    { id: "logs", label: "Logs", icon: <Terminal size={12} /> },
];

export function BottomPanel() {
    const [activeTab, setActiveTab] = useState<TabId>("queue");

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
                    <div
                        className="flex flex-col items-center justify-center"
                        style={{ height: "100%", color: "var(--text-muted)", fontSize: 12 }}
                    >
                        <History size={24} style={{ marginBottom: 8, opacity: 0.4 }} />
                        History wired to SAM <code style={{ fontSize: 10, background: "var(--bg-input)", padding: "2px 5px", borderRadius: 3 }}>historylist</code> — coming soon
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
                        <div style={{ color: "var(--green)" }}>[00:00:00] Audio engine initialized</div>
                        <div>[00:00:01] CPAL device: Built-in Output (CoreAudio)</div>
                        <div>[00:00:01] Sample rate: 44100 Hz, channels: 2</div>
                        <div style={{ color: "var(--cyan)" }}>[00:00:02] SQLite database ready</div>
                        <div style={{ color: "var(--amber)" }}>[00:00:02] Waiting for SAM MySQL connection…</div>
                    </div>
                )}
            </div>
        </div>
    );
}
