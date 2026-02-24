import { useState, useEffect, useRef } from "react";
import {
    Script,
    ScriptRunResult,
    ScriptLogEntry,
    TriggerType,
    saveScript,
    runScript,
    getScriptLog,
} from "../../lib/bridge5";

const TRIGGER_TYPES: TriggerType[] = [
    "on_track_start", "on_track_end", "on_crossfade_start",
    "on_queue_empty", "on_request_received", "on_hour",
    "on_encoder_connect", "on_encoder_disconnect", "manual",
];

const TRIGGER_LABELS: Record<TriggerType, string> = {
    on_track_start: "on_track_start — fires when a track begins",
    on_track_end: "on_track_end — fires when a track ends",
    on_crossfade_start: "on_crossfade_start — fires at crossfade begin",
    on_queue_empty: "on_queue_empty — fires when queue is empty",
    on_request_received: "on_request_received — fires on listener request",
    on_hour: "on_hour — fires at start of each hour",
    on_encoder_connect: "on_encoder_connect — fires on encoder connect",
    on_encoder_disconnect: "on_encoder_disconnect — fires on encoder disconnect",
    manual: "manual — only via Run button",
};

const DEFAULT_SCRIPT = `-- DesiZone Broadcaster — Lua Script
-- Available globals: log, store, deck, queue, media, encoder, schedule, station, http, event

log.info("Script loaded!")

-- Example: log the track title when this triggers
if event and event.title then
    log.info("Now playing: " .. event.title)
end
`;

interface Props {
    script: Script | null;  // null = create new
    onSaved: () => void;
    onClose: () => void;
}

export function ScriptEditor({ script, onSaved, onClose }: Props) {
    const isNew = !script || script.id === 0;

    const [name, setName] = useState(script?.name ?? "New Script");
    const [description] = useState(script?.description ?? "");
    const [triggerType, setTriggerType] = useState<TriggerType>(script?.trigger_type ?? "manual");
    const [enabled, setEnabled] = useState(script?.enabled ?? true);
    const [content, setContent] = useState(script?.content ?? DEFAULT_SCRIPT);

    const [running, setRunning] = useState(false);
    const [saving, setSaving] = useState(false);
    const [runResult, setRunResult] = useState<ScriptRunResult | null>(null);
    const [logEntries, setLogEntries] = useState<ScriptLogEntry[]>([]);
    const [activeTab, setActiveTab] = useState<"code" | "log">("code");
    const logRef = useRef<HTMLDivElement>(null);

    // Load log for existing script
    useEffect(() => {
        if (script && script.id) {
            getScriptLog(script.id, 50).then(setLogEntries).catch(() => { });
        }
    }, [script?.id]);

    // Auto-scroll log
    useEffect(() => {
        if (logRef.current) {
            logRef.current.scrollTop = logRef.current.scrollHeight;
        }
    }, [logEntries, runResult]);

    const handleSave = async () => {
        setSaving(true);
        try {
            const toSave: Script = {
                id: script?.id ?? 0,
                name,
                description: description || undefined,
                content,
                enabled,
                trigger_type: triggerType,
                last_run_at: script?.last_run_at,
                last_error: script?.last_error,
            };
            await saveScript(toSave);
            onSaved();
        } catch (e) {
            console.error("save failed", e);
        } finally {
            setSaving(false);
        }
    };

    const handleRun = async () => {
        // Save first so the backend runs the latest code
        setRunning(true);
        setRunResult(null);
        try {
            const toSave: Script = {
                id: script?.id ?? 0,
                name,
                description: description || undefined,
                content,
                enabled: true,
                trigger_type: triggerType,
                last_run_at: script?.last_run_at,
                last_error: script?.last_error,
            };
            await saveScript(toSave);
            const result = await runScript(script?.id ?? 0);
            setRunResult(result);
            setActiveTab("log");
            onSaved(); // refresh list
        } catch (e) {
            setRunResult({
                success: false,
                output: [],
                error: String(e),
            });
        } finally {
            setRunning(false);
        }
    };

    return (
        <div style={{
            position: "fixed", inset: 0, zIndex: 1000,
            background: "rgba(0,0,0,0.7)",
            display: "flex", alignItems: "center", justifyContent: "center",
        }}>
            <div style={{
                width: "min(900px, 96vw)",
                height: "min(680px, 90vh)",
                background: "var(--surface-1)",
                border: "1px solid var(--border)",
                borderRadius: "var(--r-lg)",
                display: "flex",
                flexDirection: "column",
                overflow: "hidden",
                boxShadow: "0 24px 64px rgba(0,0,0,0.6)",
            }}>
                {/* Header */}
                <div style={{
                    padding: "14px 20px",
                    borderBottom: "1px solid var(--border)",
                    display: "flex",
                    alignItems: "center",
                    gap: 12,
                    flexShrink: 0,
                }}>
                    <span style={{ fontSize: 14, fontWeight: 700, color: "var(--text-primary)", flex: 1 }}>
                        {isNew ? "New Script" : `Edit Script — ${script.name}`}
                    </span>
                    <button className="btn btn-ghost" style={{ padding: "3px 10px", fontSize: 11 }} onClick={onClose}>✕ Close</button>
                </div>

                {/* Meta row */}
                <div style={{
                    display: "flex",
                    gap: 12,
                    padding: "12px 20px",
                    borderBottom: "1px solid var(--border)",
                    flexShrink: 0,
                    flexWrap: "wrap",
                }}>
                    <div style={{ flex: 2, minWidth: 140 }}>
                        <label style={{ fontSize: 10, color: "var(--text-muted)", display: "block", marginBottom: 4 }}>NAME</label>
                        <input
                            className="input"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            style={{ width: "100%", fontSize: 12 }}
                        />
                    </div>
                    <div style={{ flex: 1, minWidth: 120 }}>
                        <label style={{ fontSize: 10, color: "var(--text-muted)", display: "block", marginBottom: 4 }}>TRIGGER</label>
                        <select
                            className="input"
                            value={triggerType}
                            onChange={(e) => setTriggerType(e.target.value as TriggerType)}
                            style={{ width: "100%", fontSize: 11 }}
                        >
                            {TRIGGER_TYPES.map((t) => (
                                <option key={t} value={t}>{TRIGGER_LABELS[t]}</option>
                            ))}
                        </select>
                    </div>
                    <div style={{ display: "flex", alignItems: "flex-end", gap: 8, flexShrink: 0 }}>
                        <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer", fontSize: 12 }}>
                            <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
                            Enabled
                        </label>
                    </div>
                </div>

                {/* Tabs */}
                <div style={{
                    display: "flex",
                    borderBottom: "1px solid var(--border)",
                    flexShrink: 0,
                }}>
                    {(["code", "log"] as const).map((tab) => (
                        <button
                            key={tab}
                            onClick={() => setActiveTab(tab)}
                            style={{
                                padding: "8px 16px",
                                fontSize: 11,
                                fontWeight: 600,
                                background: "none",
                                border: "none",
                                borderBottom: activeTab === tab ? "2px solid var(--cyan)" : "2px solid transparent",
                                color: activeTab === tab ? "var(--cyan)" : "var(--text-muted)",
                                cursor: "pointer",
                                transition: "color 0.15s",
                            }}
                        >
                            {tab === "code" ? "⌨ Code" : `📋 Log ${logEntries.length > 0 ? `(${logEntries.length})` : ""}`}
                        </button>
                    ))}
                </div>

                {/* Body */}
                <div style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
                    {activeTab === "code" ? (
                        <textarea
                            value={content}
                            onChange={(e) => setContent(e.target.value)}
                            spellCheck={false}
                            style={{
                                flex: 1,
                                resize: "none",
                                background: "var(--surface-0)",
                                color: "var(--text-primary)",
                                fontFamily: "var(--font-mono)",
                                fontSize: 12,
                                lineHeight: 1.6,
                                border: "none",
                                outline: "none",
                                padding: "16px 20px",
                                tabSize: 4,
                            }}
                        />
                    ) : (
                        <div
                            ref={logRef}
                            style={{
                                flex: 1,
                                overflowY: "auto",
                                padding: "12px 20px",
                                fontFamily: "var(--font-mono)",
                                fontSize: 11,
                                lineHeight: 1.8,
                            }}
                        >
                            {runResult && (
                                <div style={{
                                    marginBottom: 12,
                                    padding: "8px 12px",
                                    borderRadius: "var(--r-md)",
                                    background: runResult.success ? "rgba(34,197,94,0.1)" : "rgba(239,68,68,0.1)",
                                    border: `1px solid ${runResult.success ? "rgba(34,197,94,0.3)" : "rgba(239,68,68,0.3)"}`,
                                    color: runResult.success ? "var(--green)" : "var(--red)",
                                    fontSize: 11,
                                }}>
                                    {runResult.success ? "✓ Script ran successfully" : `✕ Error${runResult.error_line ? ` on line ${runResult.error_line}` : ""}: ${runResult.error}`}
                                </div>
                            )}
                            {logEntries.length === 0 && !runResult && (
                                <div style={{ color: "var(--text-muted)", fontStyle: "italic" }}>No log output yet. Run the script to see output.</div>
                            )}
                            {logEntries.map((entry, i) => (
                                <div key={i} style={{
                                    color: entry.level === "error" ? "var(--red)" : entry.level === "warn" ? "var(--amber)" : "var(--text-secondary)",
                                }}>
                                    <span style={{ color: "var(--text-muted)" }}>
                                        {new Date(entry.timestamp * 1000).toLocaleTimeString()}
                                    </span>{" "}
                                    <span style={{ color: entry.level === "error" ? "var(--red)" : entry.level === "warn" ? "var(--amber)" : "var(--cyan)" }}>
                                        [{entry.level.toUpperCase()}]
                                    </span>{" "}
                                    {entry.message}
                                </div>
                            ))}
                        </div>
                    )}
                </div>

                {/* Footer */}
                <div style={{
                    padding: "10px 20px",
                    borderTop: "1px solid var(--border)",
                    display: "flex",
                    justifyContent: "flex-end",
                    gap: 8,
                    flexShrink: 0,
                }}>
                    <button
                        className="btn btn-ghost"
                        style={{ padding: "5px 14px", fontSize: 11 }}
                        onClick={onClose}
                    >
                        Cancel
                    </button>
                    <button
                        className="btn btn-secondary"
                        style={{ padding: "5px 14px", fontSize: 11 }}
                        onClick={handleRun}
                        disabled={running}
                    >
                        {running ? "⟳ Running..." : "▶ Test Run"}
                    </button>
                    <button
                        className="btn btn-primary"
                        style={{ padding: "5px 14px", fontSize: 11 }}
                        onClick={handleSave}
                        disabled={saving}
                    >
                        {saving ? "Saving..." : "Save Script"}
                    </button>
                </div>
            </div>
        </div>
    );
}
