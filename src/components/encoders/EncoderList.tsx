import { useState } from "react";
import {
    EncoderConfig,
    EncoderRuntimeState,
    EncoderStatus,
    startEncoder,
    stopEncoder,
    deleteEncoder,
    saveEncoder,
} from "../../lib/bridge";
import { Plus, Trash2, Edit2, Play, Square } from "lucide-react";

interface Props {
    encoders: EncoderConfig[];
    runtime: Map<number, EncoderRuntimeState>;
    onEdit: (cfg: EncoderConfig) => void;
    onNew: () => void;
    onRefresh: () => void;
}

function isActive(status: EncoderStatus): boolean {
    if (typeof status === "string")
        return status === "streaming" || status === "recording" || status === "connecting";
    return "retrying" in status;
}

function statusBadge(status: EncoderStatus) {
    const cls = (() => {
        if (typeof status === "string") return `enc-badge enc-badge-${status}`;
        if ("retrying" in status) return "enc-badge enc-badge-retrying";
        return "enc-badge enc-badge-disabled";
    })();
    const label = (() => {
        if (typeof status === "string") return status.toUpperCase();
        if ("retrying" in status) {
            const { attempt, max } = status.retrying;
            return `RETRY ${attempt}${max > 0 ? `/${max}` : ""}`;
        }
        return "UNKNOWN";
    })();
    return <span className={cls}>{label}</span>;
}

export function EncoderList({ encoders, runtime, onEdit, onNew, onRefresh }: Props) {
    const [busy, setBusy] = useState<Set<number>>(new Set());

    const withBusy = async (id: number, fn: () => Promise<void>) => {
        setBusy((prev) => new Set(prev).add(id));
        try {
            await fn();
        } finally {
            setBusy((prev) => {
                const next = new Set(prev);
                next.delete(id);
                return next;
            });
            onRefresh();
        }
    };

    const handleToggleEnabled = async (cfg: EncoderConfig) => {
        await saveEncoder({ ...cfg, enabled: !cfg.enabled });
        onRefresh();
    };

    const handleDelete = async (cfg: EncoderConfig) => {
        if (!confirm(`Delete encoder "${cfg.name}"?`)) return;
        await deleteEncoder(cfg.id);
        onRefresh();
    };

    return (
        <div style={{ display: "flex", flexDirection: "column", gap: 0 }}>
            {/* Header */}
            <div
                style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "10px 16px 8px",
                    borderBottom: "1px solid var(--border-default)",
                    flexShrink: 0,
                }}
            >
                <span className="section-label" style={{ fontSize: 11 }}>
                    Encoders ({encoders.length})
                </span>
                <button className="btn btn-primary" style={{ padding: "4px 10px", fontSize: 11 }} onClick={onNew}>
                    <Plus size={11} /> Add Encoder
                </button>
            </div>

            {/* Encoder rows */}
            {encoders.length === 0 ? (
                <div
                    style={{
                        padding: 24,
                        textAlign: "center",
                        color: "var(--text-muted)",
                        fontSize: 12,
                    }}
                >
                    No encoders configured. Click "Add Encoder" to create one.
                </div>
            ) : (
                <div style={{ overflow: "auto", flex: 1 }}>
                    {encoders.map((cfg) => {
                        const rt = runtime.get(cfg.id);
                        const status: EncoderStatus = rt?.status ?? "disabled";
                        const active = isActive(status);
                        const isBusy = busy.has(cfg.id);

                        const outputDesc =
                            cfg.output_type === "file"
                                ? `File — ${cfg.file_rotation}`
                                : `${cfg.server_host ?? ""}:${cfg.server_port ?? ""}${cfg.mount_point ?? ""}`;

                        return (
                            <div
                                key={cfg.id}
                                className="list-row"
                                style={{
                                    borderRadius: 0,
                                    borderBottom: "1px solid var(--border-subtle)",
                                    padding: "8px 16px",
                                }}
                            >
                                {/* Toggle enabled */}
                                <div
                                    className="toggle-wrap"
                                    onClick={() => handleToggleEnabled(cfg)}
                                    style={{ flexShrink: 0 }}
                                >
                                    <div className={`toggle-track ${cfg.enabled ? "on" : ""}`}>
                                        <div className="toggle-thumb" />
                                    </div>
                                </div>

                                {/* Info */}
                                <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 2 }}>
                                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                                        <span style={{ fontWeight: 600, fontSize: 12, color: "var(--text-primary)" }}>
                                            {cfg.name}
                                        </span>
                                        {statusBadge(status)}
                                    </div>
                                    <span style={{ fontSize: 10, color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                                        {cfg.codec.toUpperCase()}
                                        {cfg.bitrate_kbps ? ` ${cfg.bitrate_kbps}kbps` : ""} — {outputDesc}
                                    </span>
                                </div>

                                {/* Listener count */}
                                {rt?.listeners != null && rt.listeners > 0 && (
                                    <span
                                        style={{
                                            fontFamily: "var(--font-mono)",
                                            fontSize: 12,
                                            color: "var(--cyan)",
                                            flexShrink: 0,
                                        }}
                                    >
                                        {rt.listeners} 👂
                                    </span>
                                )}

                                {/* Actions */}
                                <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
                                    <button
                                        className="btn btn-ghost btn-icon"
                                        title="Edit"
                                        onClick={() => onEdit(cfg)}
                                        disabled={isBusy}
                                    >
                                        <Edit2 size={12} />
                                    </button>
                                    {active ? (
                                        <button
                                            className="btn btn-danger btn-icon"
                                            title="Stop"
                                            disabled={isBusy}
                                            onClick={() => withBusy(cfg.id, () => stopEncoder(cfg.id))}
                                        >
                                            <Square size={12} />
                                        </button>
                                    ) : (
                                        <button
                                            className="btn btn-primary btn-icon"
                                            title="Start"
                                            disabled={isBusy || !cfg.enabled}
                                            onClick={() => withBusy(cfg.id, () => startEncoder(cfg.id))}
                                        >
                                            <Play size={12} />
                                        </button>
                                    )}
                                    <button
                                        className="btn btn-ghost btn-icon"
                                        title="Delete"
                                        disabled={isBusy || active}
                                        onClick={() => handleDelete(cfg)}
                                        style={{ color: "var(--red)" }}
                                    >
                                        <Trash2 size={12} />
                                    </button>
                                </div>
                            </div>
                        );
                    })}
                </div>
            )}
        </div>
    );
}
