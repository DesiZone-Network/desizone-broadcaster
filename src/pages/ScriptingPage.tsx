import { useState } from "react";
import { Script } from "../lib/bridge5";
import { ScriptList } from "../components/scripting/ScriptList";
import { ScriptEditor } from "../components/scripting/ScriptEditor";

export function ScriptingPage() {
    const [editorOpen, setEditorOpen] = useState(false);
    const [editingScript, setEditingScript] = useState<Script | null>(null);
    const [selectedId, setSelectedId] = useState<number | null>(null);
    const [refreshKey, setRefreshKey] = useState(0);

    const refresh = () => setRefreshKey((k) => k + 1);

    const handleEdit = (s: Script | null) => {
        setEditingScript(s);
        setEditorOpen(true);
    };

    const handleEditorSaved = () => {
        refresh();
    };

    const handleEditorClose = () => {
        setEditorOpen(false);
        setEditingScript(null);
    };

    return (
        <div style={{ height: "100%", display: "flex", flexDirection: "column", overflow: "hidden" }}>
            {/* Page header */}
            <div style={{
                padding: "14px 20px 12px",
                borderBottom: "1px solid var(--border)",
                flexShrink: 0,
            }}>
                <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                    ⚙ Scripting
                </h2>
                <p style={{ margin: "4px 0 0", fontSize: 11, color: "var(--text-muted)" }}>
                    Automate DesiZone Broadcaster with Lua. Scripts run in a sandboxed VM and can control decks, queues, encoders and more.
                </p>
            </div>

            {/* Body: list + maybe a selected script detail pane */}
            <div style={{
                flex: 1,
                display: "flex",
                overflow: "hidden",
            }}>
                {/* Left: script list */}
                <div style={{
                    width: "min(340px, 42%)",
                    borderRight: "1px solid var(--border)",
                    display: "flex",
                    flexDirection: "column",
                    overflow: "hidden",
                }}>
                    <ScriptList
                        onEdit={handleEdit}
                        selectedId={selectedId}
                        onSelect={setSelectedId}
                        refreshKey={refreshKey}
                    />
                </div>

                {/* Right: quick-start / tips */}
                <div style={{
                    flex: 1,
                    padding: 24,
                    overflowY: "auto",
                    display: "flex",
                    flexDirection: "column",
                    gap: 16,
                }}>
                    <div className="panel" style={{ padding: 16 }}>
                        <div className="section-label" style={{ marginBottom: 10 }}>QUICK START</div>
                        <p style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6, margin: 0 }}>
                            Click <strong style={{ color: "var(--cyan)" }}>+ New</strong> to create a script. Choose a <em>trigger</em> that determines when it runs, then write Lua code.
                        </p>
                    </div>

                    <div className="panel" style={{ padding: 16 }}>
                        <div className="section-label" style={{ marginBottom: 10 }}>GLOBAL API</div>
                        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 11, fontFamily: "var(--font-mono)" }}>
                            <tbody>
                                {[
                                    ["log.info(msg)", "Write to script log at INFO level"],
                                    ["log.warn(msg)", "Write to script log at WARN level"],
                                    ["log.error(msg)", "Write to script log at ERROR level"],
                                    ["deck.play(id)", "Resume playback on deck (\"a\" or \"b\")"],
                                    ["deck.stop(id)", "Stop playback on deck"],
                                    ["deck.load(id, song_id)", "Load a song (by SAM song ID) onto deck"],
                                    ["queue.add(song_id)", "Append song to play queue"],
                                    ["queue.clear()", "Clear the play queue"],
                                    ["encoder.start(id)", "Start an encoder by ID"],
                                    ["encoder.stop(id)", "Stop an encoder by ID"],
                                    ["http.get(url)", "HTTP GET — returns {status, body}"],
                                    ["http.post(url, body)", "HTTP POST — returns {status, body}"],
                                    ["store.get(key)", "Get a persisted script variable"],
                                    ["store.set(key, val)", "Set a persisted script variable"],
                                    ["event", "Table: current event data (fields vary by trigger)"],
                                ].map(([api, desc]) => (
                                    <tr key={api} style={{ borderBottom: "1px solid var(--border)" }}>
                                        <td style={{ padding: "5px 8px 5px 0", color: "var(--cyan)", whiteSpace: "nowrap", verticalAlign: "top" }}>{api}</td>
                                        <td style={{ padding: "5px 0 5px 12px", color: "var(--text-muted)", lineHeight: 1.5, wordBreak: "break-word" }}>{desc}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>

                    <div className="panel" style={{ padding: 16 }}>
                        <div className="section-label" style={{ marginBottom: 10 }}>EXAMPLE — HTTP NOTIFY ON TRACK START</div>
                        <pre style={{
                            margin: 0,
                            fontSize: 11,
                            fontFamily: "var(--font-mono)",
                            color: "var(--text-secondary)",
                            lineHeight: 1.7,
                            whiteSpace: "pre-wrap",
                        }}>
                            {`-- Trigger: on_track_start
local title = event.title or "Unknown"
local artist = event.artist or ""
local r = http.post("https://example.com/np", '{' ..
  '"title":"' .. title .. '",' ..
  '"artist":"' .. artist .. '"' ..
'}')
log.info("Notified: " .. r.status)`}
                        </pre>
                    </div>
                </div>
            </div>

            {/* Script Editor Modal */}
            {editorOpen && (
                <ScriptEditor
                    script={editingScript}
                    onSaved={handleEditorSaved}
                    onClose={handleEditorClose}
                />
            )}
        </div>
    );
}
