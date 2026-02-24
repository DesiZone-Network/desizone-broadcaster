import { useState, useEffect } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import * as Tabs from "@radix-ui/react-tabs";
import { X } from "lucide-react";
import { getChannelDsp, setChannelEq, setChannelAgc, ChannelDspSettings, DeckId } from "../../lib/bridge";

interface Props {
    channel: DeckId | "master";
    channelLabel: string;
    trigger?: React.ReactNode;
}

function EQSlider({
    label, value, min = -12, max = 12,
    onChange,
}: {
    label: string; value: number; min?: number; max?: number;
    onChange: (v: number) => void;
}) {
    const pct = ((value - min) / (max - min)) * 100;
    const centerPct = ((0 - min) / (max - min)) * 100;

    return (
        <div className="flex flex-col items-center gap-2" style={{ width: 60 }}>
            {/* Vertical slider track */}
            <div
                style={{
                    position: "relative",
                    width: 8,
                    height: 100,
                    background: "var(--bg-input)",
                    borderRadius: 4,
                    border: "1px solid var(--border-strong)",
                    cursor: "pointer",
                }}
                onClick={(e) => {
                    const rect = e.currentTarget.getBoundingClientRect();
                    const ratio = 1 - (e.clientY - rect.top) / rect.height;
                    onChange(Math.round((min + ratio * (max - min)) * 2) / 2);
                }}
            >
                {/* Center line */}
                <div style={{
                    position: "absolute",
                    top: `${100 - centerPct}%`,
                    left: 0, right: 0, height: 1,
                    background: "var(--border-strong)",
                }} />
                {/* Fill */}
                <div style={{
                    position: "absolute",
                    left: 1, right: 1,
                    bottom: `${Math.min(centerPct, 100 - pct)}%`,
                    height: `${Math.abs(pct - centerPct)}%`,
                    background: value > 0 ? "var(--amber)" : "var(--cyan)",
                    borderRadius: 3,
                }} />
                {/* Thumb */}
                <div style={{
                    position: "absolute",
                    top: `${100 - pct}%`,
                    left: "50%",
                    transform: "translate(-50%, -50%)",
                    width: 14,
                    height: 10,
                    background: "var(--bg-elevated)",
                    border: `2px solid ${value > 0 ? "var(--amber)" : value < 0 ? "var(--cyan)" : "var(--border-strong)"}`,
                    borderRadius: 3,
                }} />
            </div>
            <span className="mono" style={{ fontSize: 10, color: value !== 0 ? "var(--amber)" : "var(--text-muted)" }}>
                {value > 0 ? "+" : ""}{value.toFixed(1)}
            </span>
            <span className="section-label">{label}</span>
        </div>
    );
}

function EQTab({
    settings, onChange,
}: {
    settings: ChannelDspSettings;
    onChange: (updates: Partial<ChannelDspSettings>) => void;
}) {
    return (
        <div>
            <div className="flex items-center justify-center gap-6" style={{ marginTop: 20, marginBottom: 16 }}>
                <EQSlider
                    label="LOW"
                    value={settings.eq_low_gain_db}
                    onChange={(v) => onChange({ eq_low_gain_db: v })}
                />
                <EQSlider
                    label="MID"
                    value={settings.eq_mid_gain_db}
                    onChange={(v) => onChange({ eq_mid_gain_db: v })}
                />
                <EQSlider
                    label="HIGH"
                    value={settings.eq_high_gain_db}
                    onChange={(v) => onChange({ eq_high_gain_db: v })}
                />
            </div>
            <div className="separator" />
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                {[
                    { label: "Low Freq", value: settings.eq_low_freq_hz, key: "eq_low_freq_hz", min: 20, max: 500, unit: "Hz" },
                    { label: "Mid Freq", value: settings.eq_mid_freq_hz, key: "eq_mid_freq_hz", min: 200, max: 8000, unit: "Hz" },
                    { label: "Mid Q", value: settings.eq_mid_q, key: "eq_mid_q", min: 0.1, max: 5, unit: "Q" },
                    { label: "High Freq", value: settings.eq_high_freq_hz, key: "eq_high_freq_hz", min: 2000, max: 20000, unit: "Hz" },
                ].map((f) => (
                    <div key={f.key} className="form-row" style={{ padding: "4px 0" }}>
                        <span className="form-label" style={{ minWidth: 70, fontSize: 10 }}>{f.label}</span>
                        <input
                            type="range"
                            min={f.min}
                            max={f.max}
                            step={(f.max - f.min) / 100}
                            value={f.value}
                            style={{ flex: 1, accentColor: "var(--amber)" }}
                            onChange={(e) => onChange({ [f.key]: parseFloat(e.target.value) })}
                        />
                        <span className="form-value" style={{ minWidth: 50, fontSize: 10 }}>
                            {f.value >= 1000 ? `${(f.value / 1000).toFixed(1)}k` : Math.round(f.value)} {f.unit}
                        </span>
                    </div>
                ))}
            </div>
            <div style={{ marginTop: 12 }}>
                <span className="section-label">Presets</span>
                <div className="flex gap-2" style={{ marginTop: 6 }}>
                    {["Flat", "Bass Boost", "Presence", "Warm", "Bright"].map((p) => (
                        <button
                            key={p}
                            className="btn btn-ghost"
                            style={{ fontSize: 10, padding: "3px 8px" }}
                            onClick={() => {
                                if (p === "Flat") onChange({ eq_low_gain_db: 0, eq_mid_gain_db: 0, eq_high_gain_db: 0 });
                                if (p === "Bass Boost") onChange({ eq_low_gain_db: 4, eq_mid_gain_db: 0, eq_high_gain_db: -1 });
                                if (p === "Presence") onChange({ eq_low_gain_db: 0, eq_mid_gain_db: 2, eq_high_gain_db: 3 });
                                if (p === "Warm") onChange({ eq_low_gain_db: 2, eq_mid_gain_db: -0.5, eq_high_gain_db: -1.5 });
                                if (p === "Bright") onChange({ eq_low_gain_db: -1, eq_mid_gain_db: 0, eq_high_gain_db: 4 });
                            }}
                        >
                            {p}
                        </button>
                    ))}
                </div>
            </div>
        </div>
    );
}

function AGCTab({ settings, onChange }: { settings: ChannelDspSettings; onChange: (u: Partial<ChannelDspSettings>) => void }) {
    return (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <label className="checkbox-row">
                <div
                    className="checkbox-root"
                    data-state={settings.agc_enabled ? "checked" : "unchecked"}
                    onClick={() => onChange({ agc_enabled: !settings.agc_enabled })}
                    role="checkbox"
                    aria-checked={settings.agc_enabled}
                >
                    {settings.agc_enabled && (
                        <svg width="9" height="7" viewBox="0 0 9 7" fill="none">
                            <path d="M1 3.5L3.5 6L8 1" stroke="#000" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                    )}
                </div>
                <span className="checkbox-label font-medium">Gated AGC</span>
            </label>

            {[
                { label: "Gate", key: "agc_gate_db", value: settings.agc_gate_db, min: -60, max: 0, unit: "dB" },
                { label: "Max Gain", key: "agc_max_gain_db", value: settings.agc_max_gain_db, min: 0, max: 20, unit: "dB" },
                { label: "Attack", key: "agc_attack_ms", value: settings.agc_attack_ms, min: 10, max: 1000, unit: "ms" },
                { label: "Release", key: "agc_release_ms", value: settings.agc_release_ms, min: 50, max: 5000, unit: "ms" },
            ].map((f) => (
                <div key={f.key} className="form-row">
                    <span className="form-label">{f.label}</span>
                    <input
                        type="range"
                        min={f.min}
                        max={f.max}
                        step={(f.max - f.min) / 100}
                        value={f.value}
                        disabled={!settings.agc_enabled}
                        style={{ flex: 1, accentColor: "var(--amber)", opacity: settings.agc_enabled ? 1 : 0.4 }}
                        onChange={(e) => onChange({ [f.key]: parseFloat(e.target.value) })}
                    />
                    <span className="form-value">{Math.round(f.value)} {f.unit}</span>
                </div>
            ))}

            <div className="separator" />
            <div className="section-label">Pre-emphasis</div>
            <div className="flex gap-2" style={{ marginTop: 4 }}>
                {["50us", "75us", "none"].map((pe) => (
                    <button
                        key={pe}
                        className="btn btn-ghost"
                        style={{
                            fontSize: 11,
                            padding: "4px 12px",
                            background: settings.agc_pre_emphasis === pe ? "var(--amber-glow)" : "transparent",
                            borderColor: settings.agc_pre_emphasis === pe ? "var(--amber-dim)" : "var(--border-default)",
                            color: settings.agc_pre_emphasis === pe ? "var(--amber)" : "var(--text-muted)",
                        }}
                        onClick={() => onChange({ agc_pre_emphasis: pe })}
                    >
                        {pe === "none" ? "None" : pe.toUpperCase()}
                    </button>
                ))}
            </div>
        </div>
    );
}

const DEFAULT_DSP: ChannelDspSettings = {
    channel: "deck_a",
    eq_low_gain_db: 0,
    eq_low_freq_hz: 100,
    eq_mid_gain_db: 0,
    eq_mid_freq_hz: 1000,
    eq_mid_q: 0.707,
    eq_high_gain_db: 0,
    eq_high_freq_hz: 8000,
    agc_enabled: false,
    agc_gate_db: -31,
    agc_max_gain_db: 5,
    agc_attack_ms: 100,
    agc_release_ms: 500,
    agc_pre_emphasis: "75us",
    comp_enabled: false,
    comp_settings_json: null,
};

export function ChannelDspDialog({ channel, channelLabel, trigger }: Props) {
    const [settings, setSettings] = useState<ChannelDspSettings>({ ...DEFAULT_DSP, channel });
    const [open, setOpen] = useState(false);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        if (open && channel !== "master") {
            getChannelDsp(channel as DeckId)
                .then((s) => { if (s) setSettings(s); })
                .catch(console.error);
        }
    }, [open, channel]);

    const handleChange = (updates: Partial<ChannelDspSettings>) => {
        setSettings((prev) => ({ ...prev, ...updates }));
    };

    const handleSave = async () => {
        setSaving(true);
        try {
            if (channel !== "master") {
                await setChannelEq(
                    channel as DeckId,
                    settings.eq_low_gain_db,
                    settings.eq_mid_gain_db,
                    settings.eq_high_gain_db
                );
                await setChannelAgc(
                    channel as DeckId,
                    settings.agc_enabled,
                    settings.agc_gate_db,
                    settings.agc_max_gain_db
                );
            }
            setOpen(false);
        } catch (e) {
            console.error("Failed to save DSP settings:", e);
        } finally {
            setSaving(false);
        }
    };

    return (
        <Dialog.Root open={open} onOpenChange={setOpen}>
            <Dialog.Trigger asChild>
                {trigger ?? (
                    <button className="btn btn-ghost" style={{ fontSize: 11 }}>
                        DSP Settings
                    </button>
                )}
            </Dialog.Trigger>
            <Dialog.Portal>
                <Dialog.Overlay className="dialog-overlay" />
                <Dialog.Content className="dialog-content" style={{ width: 440, padding: 0 }} aria-describedby="dsp-desc">
                    {/* Header */}
                    <div
                        className="flex items-center justify-between"
                        style={{
                            padding: "14px 20px",
                            borderBottom: "1px solid var(--border-default)",
                            background: "var(--bg-surface)",
                            borderRadius: "var(--r-xl) var(--r-xl) 0 0",
                        }}
                    >
                        <Dialog.Title className="font-semibold" style={{ fontSize: 12 }}>
                            <span style={{ color: "var(--amber)" }}>{channelLabel}</span>
                            <span style={{ color: "var(--text-muted)", marginLeft: 8, fontWeight: 400 }}>Audio Settings</span>
                        </Dialog.Title>
                        <p id="dsp-desc" style={{ display: "none" }}>Configure DSP settings for {channelLabel}</p>
                        <Dialog.Close asChild>
                            <button className="btn btn-ghost btn-icon"><X size={14} /></button>
                        </Dialog.Close>
                    </div>

                    <Tabs.Root defaultValue="eq">
                        <Tabs.List className="tabs-list">
                            <Tabs.Trigger value="eq" className="tab-trigger">Equalizer</Tabs.Trigger>
                            <Tabs.Trigger value="agc" className="tab-trigger">AGC</Tabs.Trigger>
                            <Tabs.Trigger value="dsp" className="tab-trigger">DSP</Tabs.Trigger>
                        </Tabs.List>

                        <Tabs.Content value="eq" className="tab-content" style={{ minHeight: 280 }}>
                            <EQTab settings={settings} onChange={handleChange} />
                        </Tabs.Content>

                        <Tabs.Content value="agc" className="tab-content" style={{ minHeight: 280 }}>
                            <AGCTab settings={settings} onChange={handleChange} />
                        </Tabs.Content>

                        <Tabs.Content value="dsp" className="tab-content" style={{ minHeight: 280 }}>
                            <div className="flex items-center justify-center" style={{ height: 200, color: "var(--text-muted)", fontSize: 12 }}>
                                5-band compressor / multiband / clipper — coming soon
                            </div>
                        </Tabs.Content>
                    </Tabs.Root>

                    <div
                        className="flex items-center justify-end gap-2"
                        style={{
                            padding: "12px 20px",
                            borderTop: "1px solid var(--border-default)",
                            background: "var(--bg-surface)",
                            borderRadius: "0 0 var(--r-xl) var(--r-xl)",
                        }}
                    >
                        <Dialog.Close asChild>
                            <button className="btn btn-ghost" style={{ fontSize: 11 }}>Cancel</button>
                        </Dialog.Close>
                        <button className="btn btn-primary" onClick={handleSave} disabled={saving} style={{ fontSize: 11 }}>
                            {saving ? "Saving…" : "Apply"}
                        </button>
                    </div>
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
