/// MicSettings — modal for microphone configuration + Voice FX toggles

import { useState, useEffect } from "react";
import {
    MicConfig,
    AudioDevice,
    getMicConfig,
    setMicConfig,
    getAudioInputDevices,
} from "../../lib/bridge5";

interface Props {
    onClose: () => void;
}

function KnobRow({
    label, value, min, max, step, unit, onChange
}: {
    label: string; value: number; min: number; max: number; step: number; unit: string;
    onChange: (v: number) => void;
}) {
    return (
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 8 }}>
            <span style={{ fontSize: 11, color: "var(--text-muted)", minWidth: 130 }}>{label}</span>
            <input
                type="range"
                min={min} max={max} step={step} value={value}
                onChange={(e) => onChange(Number(e.target.value))}
                style={{ flex: 1 }}
            />
            <span style={{ fontSize: 11, color: "var(--text-primary)", minWidth: 48, textAlign: "right", fontFamily: "var(--font-mono)" }}>
                {value.toFixed(1)}{unit}
            </span>
        </div>
    );
}

export function MicSettings({ onClose }: Props) {
    const [devices, setDevices] = useState<AudioDevice[]>([]);
    const [cfg, setCfg] = useState<MicConfig | null>(null);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        Promise.all([getMicConfig(), getAudioInputDevices()])
            .then(([c, devs]) => { setCfg(c); setDevices(devs); })
            .catch(() => { });
    }, []);

    if (!cfg) {
        return (
            <div style={{
                position: "fixed", inset: 0, zIndex: 1100,
                background: "rgba(0,0,0,0.7)",
                display: "flex", alignItems: "center", justifyContent: "center",
            }}>
                <div className="panel" style={{ padding: 24, color: "var(--text-muted)", fontSize: 12 }}>Loading…</div>
            </div>
        );
    }

    const update = (patch: Partial<MicConfig>) => setCfg((c) => c ? { ...c, ...patch } : c);

    const handleSave = async () => {
        if (!cfg) return;
        setSaving(true);
        try {
            await setMicConfig(cfg);
            onClose();
        } catch (e) { console.error(e); }
        finally { setSaving(false); }
    };

    return (
        <div style={{
            position: "fixed", inset: 0, zIndex: 1100,
            background: "rgba(0,0,0,0.7)",
            display: "flex", alignItems: "center", justifyContent: "center",
        }}>
            <div style={{
                width: "min(560px, 94vw)",
                maxHeight: "90vh",
                overflowY: "auto",
                background: "var(--surface-1)",
                border: "1px solid var(--border)",
                borderRadius: "var(--r-lg)",
                boxShadow: "0 24px 64px rgba(0,0,0,0.6)",
            }}>
                {/* Header */}
                <div style={{
                    padding: "14px 20px",
                    borderBottom: "1px solid var(--border)",
                    display: "flex",
                    alignItems: "center",
                }}>
                    <span style={{ flex: 1, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
                        🎙 Mic / Voice FX Settings
                    </span>
                    <button className="btn btn-ghost" style={{ padding: "3px 10px", fontSize: 11 }} onClick={onClose}>✕</button>
                </div>

                <div style={{ padding: 20 }}>
                    {/* Device */}
                    <div className="section-label" style={{ marginBottom: 8 }}>INPUT DEVICE</div>
                    <select
                        className="input"
                        value={cfg.device_name ?? ""}
                        onChange={(e) => update({ device_name: e.target.value || undefined })}
                        style={{ width: "100%", fontSize: 12, marginBottom: 16 }}
                    >
                        <option value="">Default Input</option>
                        {devices.map((d) => (
                            <option key={d.name} value={d.name}>
                                {d.name}{d.is_default ? " (default)" : ""}
                            </option>
                        ))}
                    </select>

                    {/* Noise Gate */}
                    <div style={{ marginBottom: 16 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
                            <span className="section-label">NOISE GATE</span>
                            <input type="checkbox" checked={cfg.gate_enabled}
                                onChange={(e) => update({ gate_enabled: e.target.checked })} />
                        </div>
                        <div style={{ opacity: cfg.gate_enabled ? 1 : 0.4, pointerEvents: cfg.gate_enabled ? "auto" : "none" }}>
                            <KnobRow label="Threshold" value={cfg.gate_threshold_db} min={-80} max={0} step={0.5} unit=" dB" onChange={(v) => update({ gate_threshold_db: v })} />
                            <KnobRow label="Attack" value={cfg.gate_attack_ms} min={1} max={100} step={0.5} unit=" ms" onChange={(v) => update({ gate_attack_ms: v })} />
                            <KnobRow label="Release" value={cfg.gate_release_ms} min={50} max={2000} step={10} unit=" ms" onChange={(v) => update({ gate_release_ms: v })} />
                        </div>
                    </div>

                    {/* Compressor/AGC */}
                    <div style={{ marginBottom: 16 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
                            <span className="section-label">COMPRESSOR / AGC</span>
                            <input type="checkbox" checked={cfg.comp_enabled}
                                onChange={(e) => update({ comp_enabled: e.target.checked })} />
                        </div>
                        <div style={{ opacity: cfg.comp_enabled ? 1 : 0.4, pointerEvents: cfg.comp_enabled ? "auto" : "none" }}>
                            <KnobRow label="Threshold" value={cfg.comp_threshold_db} min={-60} max={0} step={0.5} unit=" dB" onChange={(v) => update({ comp_threshold_db: v })} />
                            <KnobRow label="Ratio" value={cfg.comp_ratio} min={1} max={20} step={0.25} unit=":1" onChange={(v) => update({ comp_ratio: v })} />
                            <KnobRow label="Attack" value={cfg.comp_attack_ms} min={1} max={200} step={1} unit=" ms" onChange={(v) => update({ comp_attack_ms: v })} />
                            <KnobRow label="Release" value={cfg.comp_release_ms} min={20} max={2000} step={10} unit=" ms" onChange={(v) => update({ comp_release_ms: v })} />
                        </div>
                    </div>

                    {/* PTT */}
                    <div style={{ marginBottom: 16 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
                            <span className="section-label">PUSH TO TALK</span>
                            <input type="checkbox" checked={cfg.ptt_enabled}
                                onChange={(e) => update({ ptt_enabled: e.target.checked })} />
                        </div>
                        {cfg.ptt_enabled && (
                            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                                <span style={{ fontSize: 11, color: "var(--text-muted)" }}>Hotkey:</span>
                                <input
                                    className="input"
                                    value={cfg.ptt_hotkey ?? ""}
                                    onChange={(e) => update({ ptt_hotkey: e.target.value || undefined })}
                                    placeholder="e.g. F9 or CmdOrCtrl+Shift+Space"
                                    style={{ flex: 1, fontSize: 11 }}
                                />
                            </div>
                        )}
                    </div>
                </div>

                {/* Footer */}
                <div style={{
                    padding: "10px 20px",
                    borderTop: "1px solid var(--border)",
                    display: "flex",
                    justifyContent: "flex-end",
                    gap: 8,
                }}>
                    <button className="btn btn-ghost" style={{ padding: "5px 14px", fontSize: 11 }} onClick={onClose}>Cancel</button>
                    <button className="btn btn-primary" style={{ padding: "5px 14px", fontSize: 11 }} onClick={handleSave} disabled={saving}>
                        {saving ? "Saving…" : "Save"}
                    </button>
                </div>
            </div>
        </div>
    );
}
