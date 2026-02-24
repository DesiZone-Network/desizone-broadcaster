import { useState, useEffect } from "react";
import {
    Script,
    TriggerType,
    getScripts,
    saveScript,
    deleteScript,
    runScript,
} from "../../lib/bridge5";

// ── Trigger badge ─────────────────────────────────────────────────────────────

const TRIGGER_LABELS: Record<TriggerType, string> = {
    on_track_start: "Track Start",
    on_track_end: "Track End",
    on_crossfade_start: "Crossfade",
    on_queue_empty: "Queue Empty",
    on_request_received: "Request",
    on_hour: "Hourly",
    on_encoder_connect: "Enc. Connect",
    on_encoder_disconnect: "Enc. Disconnect",
    manual: "Manual",
};

const TRIGGER_COLORS: Record<TriggerType, string> = {
    on_track_start: "var(--cyan)",
    on_track_end: "var(--text-muted)",
    on_crossfade_start: "var(--amber)",
    on_queue_empty: "var(--red)",
    on_request_received: "var(--purple)",
    on_hour: "var(--green)",
    on_encoder_connect: "var(--cyan)",
    on_encoder_disconnect: "var(--red)",
    manual: "var(--text-secondary)",
};

function TriggerBadge({ type }: { type: TriggerType }) {
    return (
        <span style={{
            fontSize: 9,
            fontFamily: "var(--font-mono)",
            padding: "2px 6px",
            borderRadius: "var(--r-sm)",
            background: `${TRIGGER_COLORS[type]}20`,
            color: TRIGGER_COLORS[type],
            border: `1px solid ${TRIGGER_COLORS[type]}40`,
            flexShrink: 0,
        }}>
            {TRIGGER_LABELS[type]}
        </span>
    );
}

// ── Script list row ───────────────────────────────────────────────────────────

interface RowProps {
    script: Script;
    selected: boolean;
    onSelect: () => void;
    onRun: () => void;
    onDelete: () => void;
    onToggle: () => void;
}

function ScriptRow({ script, selected, onSelect, onRun, onDelete, onToggle }: RowProps) {
    return (
        <div
            onClick={onSelect}
            style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "8px 12px",
                cursor: "pointer",
                borderBottom: "1px solid var(--border)",
                background: selected ? "var(--surface-2)" : "transparent",
                transition: "background 0.1s",
            }}
        >
            {/* Enable toggle */}
            <input
                type="checkbox"
                checked={script.enabled}
                onChange={(e) => { e.stopPropagation(); onToggle(); }}
                style={{ cursor: "pointer", flexShrink: 0 }}
            />

            {/* Name + info */}
            <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                    <span style={{
                        fontSize: 12,
                        fontWeight: 600,
                        color: script.enabled ? "var(--text-primary)" : "var(--text-muted)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                    }}>
                        {script.name}
                    </span>
                    <TriggerBadge type={script.trigger_type} />
                </div>
                {script.last_error && (
                    <div style={{ fontSize: 10, color: "var(--red)", marginTop: 2 }}>
                        ✕ {script.last_error.slice(0, 60)}
                    </div>
                )}
                {script.last_run_at && !script.last_error && (
                    <div style={{ fontSize: 10, color: "var(--text-muted)", marginTop: 2 }}>
                        Last run: {new Date(script.last_run_at * 1000).toLocaleTimeString()}
                    </div>
                )}
            </div>

            {/* Actions */}
            <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
                <button
                    className="btn btn-ghost"
                    style={{ padding: "2px 7px", fontSize: 10 }}
                    onClick={(e) => { e.stopPropagation(); onRun(); }}
                    title="Run manually"
                >
                    ▶
                </button>
                <button
                    className="btn btn-ghost"
                    style={{ padding: "2px 7px", fontSize: 10, color: "var(--red)" }}
                    onClick={(e) => { e.stopPropagation(); onDelete(); }}
                    title="Delete"
                >
                    ✕
                </button>
            </div>
        </div>
    );
}

// ── Props ─────────────────────────────────────────────────────────────────────

interface Props {
    onEdit: (script: Script | null) => void;
    selectedId: number | null;
    onSelect: (id: number) => void;
    refreshKey: number;
}

// ── ScriptList ────────────────────────────────────────────────────────────────

export function ScriptList({ onEdit, selectedId, onSelect, refreshKey }: Props) {
    const [scripts, setScripts] = useState<Script[]>([]);

    const load = async () => {
        try { setScripts(await getScripts()); } catch { /* ignore */ }
    };

    useEffect(() => { load(); }, [refreshKey]);

    const handleRun = async (s: Script) => {
        try { await runScript(s.id); } catch { /* ignore */ }
        finally { await load(); }
    };

    const handleDelete = async (id: number) => {
        if (!confirm("Delete this script?")) return;
        await deleteScript(id);
        await load();
    };

    const handleToggle = async (s: Script) => {
        await saveScript({ ...s, enabled: !s.enabled });
        await load();
    };

    return (
        <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
            {/* Toolbar */}
            <div style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "10px 12px 8px",
                borderBottom: "1px solid var(--border)",
                flexShrink: 0,
            }}>
                <span className="section-label">Scripts</span>
                <button
                    className="btn btn-primary"
                    style={{ padding: "3px 10px", fontSize: 11 }}
                    onClick={() => onEdit(null)}
                >
                    + New
                </button>
            </div>

            {/* List */}
            <div style={{ flex: 1, overflowY: "auto" }}>
                {scripts.length === 0 && (
                    <div style={{
                        display: "flex", alignItems: "center", justifyContent: "center",
                        height: 80, color: "var(--text-muted)", fontSize: 11,
                    }}>
                        No scripts yet
                    </div>
                )}
                {scripts.map((s) => (
                    <ScriptRow
                        key={s.id}
                        script={s}
                        selected={selectedId === s.id}
                        onSelect={() => onSelect(s.id)}
                        onRun={() => handleRun(s)}
                        onDelete={() => handleDelete(s.id)}
                        onToggle={() => handleToggle(s)}
                    />
                ))}
            </div>
        </div>
    );
}
