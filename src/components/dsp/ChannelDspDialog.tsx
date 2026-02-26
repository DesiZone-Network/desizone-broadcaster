import { useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import * as Tabs from "@radix-ui/react-tabs";
import { X } from "lucide-react";
import {
    DeckId,
    PipelineSettings,
    getChannelDsp,
    setPipelineSettings,
} from "../../lib/bridge";

interface Props {
    channel: DeckId | "master";
    channelLabel: string;
    trigger?: React.ReactNode;
}

const DEFAULT_PIPELINE: PipelineSettings = {
    eq: {
        low_gain_db: 0,
        low_freq_hz: 100,
        mid_gain_db: 0,
        mid_freq_hz: 1000,
        mid_q: 0.707,
        high_gain_db: 0,
        high_freq_hz: 8000,
    },
    agc: {
        enabled: false,
        gate_db: -31,
        max_gain_db: 5,
        target_db: -18,
        attack_ms: 100,
        release_ms: 500,
        pre_emphasis: "us75",
    },
    multiband: {
        enabled: false,
        bands: Array.from({ length: 5 }, () => ({
            threshold_db: -20,
            ratio: 3,
            knee_db: 6,
            attack_ms: 5,
            release_ms: 50,
            makeup_db: 0,
        })),
    },
    dual_band: {
        enabled: false,
        crossover_hz: 800,
        lf_band: {
            threshold_db: -18,
            ratio: 4,
            knee_db: 6,
            attack_ms: 5,
            release_ms: 50,
            makeup_db: 0,
        },
        hf_band: {
            threshold_db: -18,
            ratio: 3,
            knee_db: 6,
            attack_ms: 5,
            release_ms: 50,
            makeup_db: 0,
        },
    },
    clipper: {
        enabled: true,
        ceiling_db: -0.1,
    },
    stem_filter: {
        mode: "off",
        amount: 0.85,
    },
};

function copyPipeline(p: PipelineSettings): PipelineSettings {
    return JSON.parse(JSON.stringify(p));
}

function defaultPipelineForChannel(channel: DeckId | "master"): PipelineSettings {
    const base = copyPipeline(DEFAULT_PIPELINE);
    if (channel === "deck_a" || channel === "deck_b") {
        base.stem_filter.amount = 0.82;
    } else if (channel === "voice_fx") {
        base.stem_filter.amount = 0.55;
    } else {
        base.stem_filter.amount = 0.70;
    }
    return base;
}

function normalizePipeline(p: PipelineSettings): PipelineSettings {
    const base = copyPipeline(DEFAULT_PIPELINE);
    return {
        ...base,
        ...p,
        eq: { ...base.eq, ...p.eq },
        agc: { ...base.agc, ...p.agc },
        multiband: {
            ...base.multiband,
            ...p.multiband,
            bands: p.multiband?.bands?.length ? p.multiband.bands : base.multiband.bands,
        },
        dual_band: {
            ...base.dual_band,
            ...p.dual_band,
            lf_band: { ...base.dual_band.lf_band, ...p.dual_band?.lf_band },
            hf_band: { ...base.dual_band.hf_band, ...p.dual_band?.hf_band },
        },
        clipper: { ...base.clipper, ...p.clipper },
        stem_filter: { ...base.stem_filter, ...p.stem_filter },
    };
}

function applyBandToAll(
    settings: PipelineSettings,
    updater: (band: PipelineSettings["multiband"]["bands"][number]) => void
): PipelineSettings {
    const next = copyPipeline(settings);
    next.multiband.bands = next.multiband.bands.map((b) => {
        const cloned = { ...b };
        updater(cloned);
        return cloned;
    });
    return next;
}

function legacyFallbackToPipeline(row: any, channel: DeckId | "master"): PipelineSettings {
    const p = defaultPipelineForChannel(channel);
    p.eq.low_gain_db = row.eq_low_gain_db ?? p.eq.low_gain_db;
    p.eq.low_freq_hz = row.eq_low_freq_hz ?? p.eq.low_freq_hz;
    p.eq.mid_gain_db = row.eq_mid_gain_db ?? p.eq.mid_gain_db;
    p.eq.mid_freq_hz = row.eq_mid_freq_hz ?? p.eq.mid_freq_hz;
    p.eq.mid_q = row.eq_mid_q ?? p.eq.mid_q;
    p.eq.high_gain_db = row.eq_high_gain_db ?? p.eq.high_gain_db;
    p.eq.high_freq_hz = row.eq_high_freq_hz ?? p.eq.high_freq_hz;

    p.agc.enabled = !!row.agc_enabled;
    p.agc.gate_db = row.agc_gate_db ?? p.agc.gate_db;
    p.agc.max_gain_db = row.agc_max_gain_db ?? p.agc.max_gain_db;
    p.agc.attack_ms = row.agc_attack_ms ?? p.agc.attack_ms;
    p.agc.release_ms = row.agc_release_ms ?? p.agc.release_ms;
    p.agc.pre_emphasis =
        row.agc_pre_emphasis === "50us"
            ? "us50"
            : row.agc_pre_emphasis === "75us"
            ? "us75"
            : "none";

    p.multiband.enabled = !!row.comp_enabled;
    return p;
}

function NumberSlider({
    label,
    value,
    min,
    max,
    step,
    unit,
    accent = "var(--amber)",
    onChange,
    disabled,
}: {
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    unit?: string;
    accent?: string;
    onChange: (v: number) => void;
    disabled?: boolean;
}) {
    return (
        <div className="form-row">
            <span className="form-label">{label}</span>
            <input
                type="range"
                min={min}
                max={max}
                step={step}
                value={value}
                disabled={disabled}
                onChange={(e) => onChange(parseFloat(e.target.value))}
                style={{ flex: 1, accentColor: accent, opacity: disabled ? 0.45 : 1 }}
            />
            <span className="form-value">
                {Number.isFinite(value) ? value.toFixed(step < 1 ? 1 : 0) : "0"}
                {unit ? ` ${unit}` : ""}
            </span>
        </div>
    );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
    return (
        <label className="checkbox-row">
            <div
                className="checkbox-root"
                data-state={checked ? "checked" : "unchecked"}
                onClick={() => onChange(!checked)}
                role="checkbox"
                aria-checked={checked}
            >
                {checked && (
                    <svg width="9" height="7" viewBox="0 0 9 7" fill="none">
                        <path d="M1 3.5L3.5 6L8 1" stroke="#000" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                )}
            </div>
            <span className="checkbox-label">{label}</span>
        </label>
    );
}

export function ChannelDspDialog({ channel, channelLabel, trigger }: Props) {
    const [settings, setSettings] = useState<PipelineSettings>(defaultPipelineForChannel(channel));
    const [open, setOpen] = useState(false);
    const [saving, setSaving] = useState(false);

    const applyStemPreset = (preset: "off" | "vocal_boost" | "karaoke" | "light_isolation") => {
        setSettings((prev) => {
            const next = copyPipeline(prev);
            switch (preset) {
                case "off":
                    next.stem_filter.mode = "off";
                    break;
                case "vocal_boost":
                    next.stem_filter.mode = "vocal";
                    next.stem_filter.amount = 0.72;
                    break;
                case "karaoke":
                    next.stem_filter.mode = "instrumental";
                    next.stem_filter.amount = 0.90;
                    break;
                case "light_isolation":
                    next.stem_filter.mode = "instrumental";
                    next.stem_filter.amount = 0.55;
                    break;
                default:
                    break;
            }
            return next;
        });
    };

    useEffect(() => {
        if (!open) return;
        getChannelDsp(channel)
            .then((row) => {
                if (!row) {
                    setSettings(defaultPipelineForChannel(channel));
                    return;
                }
                if (row.pipeline_settings_json) {
                    try {
                        const parsed = JSON.parse(row.pipeline_settings_json) as PipelineSettings;
                        setSettings(normalizePipeline(parsed));
                        return;
                    } catch {
                        // Fall through to legacy hydration.
                    }
                }
                setSettings(legacyFallbackToPipeline(row, channel));
            })
            .catch(() => {
                setSettings(defaultPipelineForChannel(channel));
            });
    }, [open, channel]);

    const save = async () => {
        setSaving(true);
        try {
            await setPipelineSettings(channel, settings);
            setOpen(false);
        } catch (e) {
            console.error("Failed to save pipeline settings", e);
        } finally {
            setSaving(false);
        }
    };

    return (
        <Dialog.Root open={open} onOpenChange={setOpen}>
            <Dialog.Trigger asChild>
                {trigger ?? <button className="btn btn-ghost">DSP Settings</button>}
            </Dialog.Trigger>

            <Dialog.Portal>
                <Dialog.Overlay className="dialog-overlay" />
                <Dialog.Content className="dialog-content" style={{ width: 520, padding: 0 }} aria-describedby="dsp-desc">
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
                            <span style={{ color: "var(--text-muted)", marginLeft: 8, fontWeight: 400 }}>Pipeline Settings</span>
                        </Dialog.Title>
                        <p id="dsp-desc" style={{ display: "none" }}>
                            Configure EQ, AGC, multiband, dual-band and clipper stages for {channelLabel}
                        </p>
                        <Dialog.Close asChild>
                            <button className="btn btn-ghost btn-icon"><X size={14} /></button>
                        </Dialog.Close>
                    </div>

                    <Tabs.Root defaultValue="eq">
                        <Tabs.List className="tabs-list">
                            <Tabs.Trigger value="eq" className="tab-trigger">EQ</Tabs.Trigger>
                            <Tabs.Trigger value="agc" className="tab-trigger">AGC</Tabs.Trigger>
                            <Tabs.Trigger value="dsp" className="tab-trigger">Multiband / Dual / Clipper</Tabs.Trigger>
                        </Tabs.List>

                        <Tabs.Content value="eq" className="tab-content" style={{ minHeight: 320 }}>
                            <NumberSlider label="Low Gain" value={settings.eq.low_gain_db} min={-12} max={12} step={0.5} unit="dB" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, low_gain_db: v } }))} />
                            <NumberSlider label="Low Freq" value={settings.eq.low_freq_hz} min={20} max={500} step={1} unit="Hz" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, low_freq_hz: v } }))} />
                            <NumberSlider label="Mid Gain" value={settings.eq.mid_gain_db} min={-12} max={12} step={0.5} unit="dB" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, mid_gain_db: v } }))} />
                            <NumberSlider label="Mid Freq" value={settings.eq.mid_freq_hz} min={200} max={8000} step={1} unit="Hz" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, mid_freq_hz: v } }))} />
                            <NumberSlider label="Mid Q" value={settings.eq.mid_q} min={0.1} max={5} step={0.1} unit="Q" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, mid_q: v } }))} />
                            <NumberSlider label="High Gain" value={settings.eq.high_gain_db} min={-12} max={12} step={0.5} unit="dB" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, high_gain_db: v } }))} />
                            <NumberSlider label="High Freq" value={settings.eq.high_freq_hz} min={2000} max={20000} step={10} unit="Hz" onChange={(v) => setSettings((s) => ({ ...s, eq: { ...s.eq, high_freq_hz: v } }))} />
                        </Tabs.Content>

                        <Tabs.Content value="agc" className="tab-content" style={{ minHeight: 320 }}>
                            <Toggle
                                checked={settings.agc.enabled}
                                onChange={(v) => setSettings((s) => ({ ...s, agc: { ...s.agc, enabled: v } }))}
                                label="Enable AGC"
                            />
                            <NumberSlider label="Gate" value={settings.agc.gate_db} min={-60} max={0} step={0.5} unit="dB" disabled={!settings.agc.enabled} onChange={(v) => setSettings((s) => ({ ...s, agc: { ...s.agc, gate_db: v } }))} />
                            <NumberSlider label="Max Gain" value={settings.agc.max_gain_db} min={0} max={20} step={0.5} unit="dB" disabled={!settings.agc.enabled} onChange={(v) => setSettings((s) => ({ ...s, agc: { ...s.agc, max_gain_db: v } }))} />
                            <NumberSlider label="Target" value={settings.agc.target_db} min={-30} max={-6} step={0.5} unit="dB" disabled={!settings.agc.enabled} onChange={(v) => setSettings((s) => ({ ...s, agc: { ...s.agc, target_db: v } }))} />
                            <NumberSlider label="Attack" value={settings.agc.attack_ms} min={10} max={1000} step={1} unit="ms" disabled={!settings.agc.enabled} onChange={(v) => setSettings((s) => ({ ...s, agc: { ...s.agc, attack_ms: v } }))} />
                            <NumberSlider label="Release" value={settings.agc.release_ms} min={50} max={5000} step={5} unit="ms" disabled={!settings.agc.enabled} onChange={(v) => setSettings((s) => ({ ...s, agc: { ...s.agc, release_ms: v } }))} />
                            <div className="flex items-center gap-2" style={{ marginTop: 8 }}>
                                <span className="form-label">Pre-emphasis</span>
                                {(["none", "us50", "us75"] as const).map((m) => (
                                    <button
                                        key={m}
                                        className="btn btn-ghost"
                                        style={{
                                            fontSize: 10,
                                            padding: "3px 8px",
                                            background: settings.agc.pre_emphasis === m ? "var(--amber-glow)" : "transparent",
                                            borderColor: settings.agc.pre_emphasis === m ? "var(--amber-dim)" : "var(--border-default)",
                                            color: settings.agc.pre_emphasis === m ? "var(--amber)" : "var(--text-muted)",
                                        }}
                                        onClick={() => setSettings((s) => ({ ...s, agc: { ...s.agc, pre_emphasis: m } }))}
                                    >
                                        {m.toUpperCase()}
                                    </button>
                                ))}
                            </div>
                        </Tabs.Content>

                        <Tabs.Content value="dsp" className="tab-content" style={{ minHeight: 320 }}>
                            <Toggle
                                checked={settings.multiband.enabled}
                                onChange={(v) => setSettings((s) => ({ ...s, multiband: { ...s.multiband, enabled: v } }))}
                                label="Enable 5-band multiband compressor"
                            />
                            <NumberSlider
                                label="MB Threshold"
                                value={settings.multiband.bands[0]?.threshold_db ?? -20}
                                min={-40}
                                max={0}
                                step={0.5}
                                unit="dB"
                                disabled={!settings.multiband.enabled}
                                onChange={(v) => setSettings((s) => applyBandToAll(s, (b) => { b.threshold_db = v; }))}
                            />
                            <NumberSlider
                                label="MB Ratio"
                                value={settings.multiband.bands[0]?.ratio ?? 3}
                                min={1}
                                max={20}
                                step={0.1}
                                disabled={!settings.multiband.enabled}
                                onChange={(v) => setSettings((s) => applyBandToAll(s, (b) => { b.ratio = v; }))}
                            />
                            <NumberSlider
                                label="MB Makeup"
                                value={settings.multiband.bands[0]?.makeup_db ?? 0}
                                min={-6}
                                max={12}
                                step={0.1}
                                unit="dB"
                                disabled={!settings.multiband.enabled}
                                onChange={(v) => setSettings((s) => applyBandToAll(s, (b) => { b.makeup_db = v; }))}
                            />

                            <div className="separator" />

                            <Toggle
                                checked={settings.dual_band.enabled}
                                onChange={(v) => setSettings((s) => ({ ...s, dual_band: { ...s.dual_band, enabled: v } }))}
                                label="Enable dual-band compressor"
                            />
                            <NumberSlider label="Dual Crossover" value={settings.dual_band.crossover_hz} min={80} max={4000} step={1} unit="Hz" disabled={!settings.dual_band.enabled} onChange={(v) => setSettings((s) => ({ ...s, dual_band: { ...s.dual_band, crossover_hz: v } }))} />
                            <NumberSlider label="LF Ratio" value={settings.dual_band.lf_band.ratio} min={1} max={20} step={0.1} disabled={!settings.dual_band.enabled} onChange={(v) => setSettings((s) => ({ ...s, dual_band: { ...s.dual_band, lf_band: { ...s.dual_band.lf_band, ratio: v } } }))} />
                            <NumberSlider label="HF Ratio" value={settings.dual_band.hf_band.ratio} min={1} max={20} step={0.1} disabled={!settings.dual_band.enabled} onChange={(v) => setSettings((s) => ({ ...s, dual_band: { ...s.dual_band, hf_band: { ...s.dual_band.hf_band, ratio: v } } }))} />

                            <div className="separator" />

                            <Toggle
                                checked={settings.clipper.enabled}
                                onChange={(v) => setSettings((s) => ({ ...s, clipper: { ...s.clipper, enabled: v } }))}
                                label="Enable clipper"
                            />
                            <NumberSlider label="Ceiling" value={settings.clipper.ceiling_db} min={-6} max={0} step={0.1} unit="dB" disabled={!settings.clipper.enabled} onChange={(v) => setSettings((s) => ({ ...s, clipper: { ...s.clipper, ceiling_db: v } }))} />

                            <div className="separator" />

                            <div className="flex items-center gap-2" style={{ marginTop: 6 }}>
                                <span className="form-label">Vocal / Instrument Filter</span>
                                {(
                                    [
                                        { value: "off", label: "OFF" },
                                        { value: "vocal", label: "VOCAL" },
                                        { value: "instrumental", label: "INSTR" },
                                    ] as const
                                ).map((m) => (
                                    <button
                                        key={m.value}
                                        className="btn btn-ghost"
                                        style={{
                                            fontSize: 10,
                                            padding: "3px 8px",
                                            background: settings.stem_filter.mode === m.value ? "var(--amber-glow)" : "transparent",
                                            borderColor: settings.stem_filter.mode === m.value ? "var(--amber-dim)" : "var(--border-default)",
                                            color: settings.stem_filter.mode === m.value ? "var(--amber)" : "var(--text-muted)",
                                        }}
                                        onClick={() => setSettings((s) => ({ ...s, stem_filter: { ...s.stem_filter, mode: m.value } }))}
                                    >
                                        {m.label}
                                    </button>
                                ))}
                            </div>
                            <div className="flex items-center gap-2" style={{ marginTop: 6, flexWrap: "wrap" }}>
                                <span className="form-label">Presets</span>
                                {(
                                    [
                                        { key: "vocal_boost", label: "Vocal Boost" },
                                        { key: "karaoke", label: "Karaoke" },
                                        { key: "light_isolation", label: "Light Isolation" },
                                        { key: "off", label: "Filter Off" },
                                    ] as const
                                ).map((p) => (
                                    <button
                                        key={p.key}
                                        className="btn btn-ghost"
                                        style={{ fontSize: 10, padding: "3px 8px" }}
                                        onClick={() => applyStemPreset(p.key)}
                                    >
                                        {p.label}
                                    </button>
                                ))}
                            </div>
                            <NumberSlider
                                label="Filter Amount"
                                value={Math.round((settings.stem_filter.amount ?? 0) * 100)}
                                min={0}
                                max={100}
                                step={1}
                                unit="%"
                                disabled={settings.stem_filter.mode === "off"}
                                onChange={(v) => setSettings((s) => ({ ...s, stem_filter: { ...s.stem_filter, amount: Math.max(0, Math.min(1, v / 100)) } }))}
                            />
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
                        <button className="btn btn-primary" onClick={save} disabled={saving} style={{ fontSize: 11 }}>
                            {saving ? "Saving..." : "Apply"}
                        </button>
                    </div>
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
