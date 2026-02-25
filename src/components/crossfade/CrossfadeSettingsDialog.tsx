import { useState, useEffect } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import * as Select from "@radix-ui/react-select";
import { X, ChevronDown, RotateCcw } from "lucide-react";
import {
    AutodjTransitionEngine,
    getAutoDjTransitionConfig,
    getCrossfadeConfig,
    setAutoDjTransitionConfig,
    setCrossfadeConfig,
    recalculateAutoDjPlanNow,
    AutoTransitionConfig,
    AutoTransitionMode,
    CrossfadeConfig,
    CrossfadeTriggerMode,
    FadeCurve,
} from "../../lib/bridge";
import { FadeCurveGraph } from "./FadeCurveGraph";

const CURVES: { value: FadeCurve; label: string }[] = [
    { value: "linear", label: "Linear" },
    { value: "exponential", label: "Exponential" },
    { value: "s_curve", label: "S-Curve" },
    { value: "logarithmic", label: "Logarithmic" },
    { value: "constant_power", label: "Constant Power" },
];

const DEFAULT_CONFIG: CrossfadeConfig = {
    fade_out_enabled: true,
    fade_out_curve: "exponential",
    fade_out_time_ms: 10000,
    fade_out_level_pct: 80,
    fade_in_enabled: true,
    fade_in_curve: "s_curve",
    fade_in_time_ms: 10000,
    fade_in_level_pct: 80,
    crossfade_mode: "overlap",
    trigger_mode: "auto_detect_db",
    fixed_crossfade_ms: 8000,
    auto_detect_enabled: true,
    auto_detect_db: -3,
    min_fade_time_ms: 3000,
    max_fade_time_ms: 10000,
    skip_short_tracks_secs: 65,
    auto_detect_min_ms: 500,
    auto_detect_max_ms: 15000,
    fixed_crossfade_point_ms: 8000,
};

const DEFAULT_AUTO_TRANSITION: AutoTransitionConfig = {
    engine: "sam_classic",
    mixxx_planner_config: {
        enabled: true,
        mode: "full_intro_outro",
        transition_time_sec: 10,
        min_track_duration_ms: 200,
    },
};

const AUTO_MODES: { value: AutoTransitionMode; label: string }[] = [
    { value: "full_intro_outro", label: "Full Intro + Outro" },
    { value: "fade_at_outro_start", label: "Fade At Outro Start" },
    { value: "fixed_full_track", label: "Full Track" },
    { value: "fixed_skip_silence", label: "Skip Silence" },
    { value: "fixed_start_center_skip_silence", label: "Skip Silence Start Full Volume" },
];

const TRIGGER_MODES: { value: CrossfadeTriggerMode; label: string }[] = [
    { value: "auto_detect_db", label: "Auto detect (dB level)" },
    { value: "fixed_point_ms", label: "Fixed cross-fade point" },
    { value: "manual", label: "Manual" },
];

const AUTODJ_ENGINES: { value: AutodjTransitionEngine; label: string }[] = [
    { value: "sam_classic", label: "SAM Classic (Default)" },
    { value: "mixxx_planner", label: "Mixxx Planner (Advanced)" },
];

function StyledSlider({
    value, min, max, step = 1, onChange, label, unit = "ms",
}: {
    value: number; min: number; max: number; step?: number;
    onChange: (v: number) => void; label: string; unit?: string;
}) {
    const pct = ((value - min) / (max - min)) * 100;

    const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
        const rect = e.currentTarget.getBoundingClientRect();
        const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        onChange(Math.round((min + ratio * (max - min)) / step) * step);
    };

    return (
        <div className="flex items-center gap-3">
            <span className="form-label" style={{ minWidth: 80, fontSize: 11 }}>{label}</span>
            <div
                className="slider-track"
                style={{ flex: 1, cursor: "pointer" }}
                onClick={handleClick}
            >
                <div className="slider-range" style={{ width: `${pct}%` }} />
            </div>
            <span className="form-value" style={{ minWidth: 70, textAlign: "right" }}>
                {value.toLocaleString()} {unit}
            </span>
        </div>
    );
}

function CurveSelect({ value, onChange }: { value: FadeCurve; onChange: (v: FadeCurve) => void }) {
    return (
        <Select.Root value={value} onValueChange={(v) => onChange(v as FadeCurve)}>
            <Select.Trigger className="select-trigger" aria-label="Fade curve">
                <Select.Value />
                <ChevronDown size={12} />
            </Select.Trigger>
            <Select.Portal>
                <Select.Content className="select-content" position="popper" sideOffset={4}>
                    <Select.Viewport>
                        {CURVES.map((c) => (
                            <Select.Item key={c.value} value={c.value} className="select-item">
                                <Select.ItemText>{c.label}</Select.ItemText>
                            </Select.Item>
                        ))}
                    </Select.Viewport>
                </Select.Content>
            </Select.Portal>
        </Select.Root>
    );
}

function Checkbox({
    checked, onCheckedChange, label,
}: { checked: boolean; onCheckedChange: (v: boolean) => void; label: string }) {
    return (
        <label className="checkbox-row">
            <div
                className="checkbox-root"
                data-state={checked ? "checked" : "unchecked"}
                onClick={() => onCheckedChange(!checked)}
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

interface Props {
    trigger?: React.ReactNode;
}

export function CrossfadeSettingsDialog({ trigger }: Props) {
    const [config, setConfig] = useState<CrossfadeConfig>(DEFAULT_CONFIG);
    const [autoTransition, setAutoTransition] = useState<AutoTransitionConfig>(DEFAULT_AUTO_TRANSITION);
    const [open, setOpen] = useState(false);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        if (open) {
            Promise.all([getCrossfadeConfig(), getAutoDjTransitionConfig()])
                .then(([crossfadeCfg, autoCfg]) => {
                    setConfig(crossfadeCfg);
                    setAutoTransition(autoCfg);
                })
                .catch(() => { }); // use defaults on error
        }
    }, [open]);

    const update = <K extends keyof CrossfadeConfig>(key: K, value: CrossfadeConfig[K]) => {
        setConfig((prev) => ({ ...prev, [key]: value }));
    };

    const handleSave = async () => {
        setSaving(true);
        try {
            await Promise.all([
                setCrossfadeConfig(config),
                setAutoDjTransitionConfig(autoTransition),
            ]);
            await recalculateAutoDjPlanNow();
            setOpen(false);
        } catch (e) {
            console.error("Failed to save crossfade config:", e);
        } finally {
            setSaving(false);
        }
    };

    const handleReset = () => {
        setConfig(DEFAULT_CONFIG);
        setAutoTransition(DEFAULT_AUTO_TRANSITION);
    };

    return (
        <Dialog.Root open={open} onOpenChange={setOpen}>
            <Dialog.Trigger asChild>
                {trigger ?? (
                    <button className="btn btn-ghost" style={{ fontSize: 11 }}>
                        Crossfade Settings
                    </button>
                )}
            </Dialog.Trigger>

            <Dialog.Portal>
                <Dialog.Overlay className="dialog-overlay" />
                <Dialog.Content
                    className="dialog-content"
                    style={{ width: 560, padding: 0 }}
                    aria-describedby="xfade-desc"
                >
                    {/* Dialog header */}
                    <div
                        className="flex items-center justify-between"
                        style={{
                            padding: "14px 20px",
                            borderBottom: "1px solid var(--border-default)",
                            background: "var(--bg-surface)",
                            borderRadius: "var(--r-xl) var(--r-xl) 0 0",
                        }}
                    >
                        <Dialog.Title
                            className="font-semibold tracking-wide uppercase"
                            style={{ fontSize: 12, color: "var(--amber)" }}
                        >
                            Cross-Fading
                        </Dialog.Title>
                        <p id="xfade-desc" style={{ display: "none" }}>Configure crossfade settings</p>
                        <Dialog.Close asChild>
                            <button className="btn btn-ghost btn-icon">
                                <X size={14} />
                            </button>
                        </Dialog.Close>
                    </div>

                    <div style={{ padding: 20 }}>
                        {/* Two column layout: Fade Out | Fade In */}
                        <div className="flex gap-5">
                            {/* Fade Out */}
                            <div style={{ flex: 1 }}>
                                <div
                                    className="section-label"
                                    style={{ marginBottom: 10, color: "var(--amber)" }}
                                >
                                    Fade Out
                                </div>
                                <Checkbox
                                    checked={config.fade_out_enabled}
                                    onCheckedChange={(v) => update("fade_out_enabled", v)}
                                    label="Enable fade out"
                                />
                                <div className="form-row" style={{ marginTop: 10 }}>
                                    <span className="form-label">Curve</span>
                                    <CurveSelect
                                        value={config.fade_out_curve}
                                        onChange={(v) => update("fade_out_curve", v)}
                                    />
                                </div>
                                <div style={{ marginTop: 8 }}>
                                    <StyledSlider
                                        label="Time"
                                        value={config.fade_out_time_ms}
                                        min={500}
                                        max={15000}
                                        step={500}
                                        onChange={(v) => update("fade_out_time_ms", v)}
                                    />
                                </div>
                            </div>

                            {/* Divider */}
                            <div style={{ width: 1, background: "var(--border-default)", margin: "0 4px" }} />

                            {/* Fade In */}
                            <div style={{ flex: 1 }}>
                                <div
                                    className="section-label"
                                    style={{ marginBottom: 10, color: "var(--cyan)" }}
                                >
                                    Fade In
                                </div>
                                <Checkbox
                                    checked={config.fade_in_enabled}
                                    onCheckedChange={(v) => update("fade_in_enabled", v)}
                                    label="Enable fade in"
                                />
                                <div className="form-row" style={{ marginTop: 10 }}>
                                    <span className="form-label">Curve</span>
                                    <CurveSelect
                                        value={config.fade_in_curve}
                                        onChange={(v) => update("fade_in_curve", v)}
                                    />
                                </div>
                                <div style={{ marginTop: 8 }}>
                                    <StyledSlider
                                        label="Time"
                                        value={config.fade_in_time_ms}
                                        min={500}
                                        max={15000}
                                        step={500}
                                        onChange={(v) => update("fade_in_time_ms", v)}
                                    />
                                </div>
                            </div>
                        </div>

                        <div className="separator" />

                        {/* Cross-fade section */}
                        <div className="section-label" style={{ marginBottom: 10 }}>Cross-fade</div>

                        <div className="flex items-center gap-3 form-row">
                            <span className="form-label">Blend</span>
                            <Select.Root
                                value={config.crossfade_mode}
                                onValueChange={(v) => update("crossfade_mode", v as CrossfadeConfig["crossfade_mode"])}
                            >
                                <Select.Trigger className="select-trigger">
                                    <Select.Value />
                                    <ChevronDown size={12} />
                                </Select.Trigger>
                                <Select.Portal>
                                    <Select.Content className="select-content" position="popper" sideOffset={4}>
                                        <Select.Viewport>
                                            {[
                                                { value: "overlap", label: "Overlap" },
                                                { value: "segue", label: "Segue" },
                                                { value: "instant", label: "Instant cut" },
                                            ].map((m) => (
                                                <Select.Item key={m.value} value={m.value} className="select-item">
                                                    <Select.ItemText>{m.label}</Select.ItemText>
                                                </Select.Item>
                                            ))}
                                        </Select.Viewport>
                                    </Select.Content>
                                </Select.Portal>
                            </Select.Root>
                        </div>

                        <div className="flex items-center gap-3 form-row" style={{ marginTop: 8 }}>
                            <span className="form-label">Trigger</span>
                            <Select.Root
                                value={config.trigger_mode}
                                onValueChange={(v) => update("trigger_mode", v as CrossfadeTriggerMode)}
                            >
                                <Select.Trigger className="select-trigger">
                                    <Select.Value />
                                    <ChevronDown size={12} />
                                </Select.Trigger>
                                <Select.Portal>
                                    <Select.Content className="select-content" position="popper" sideOffset={4}>
                                        <Select.Viewport>
                                            {TRIGGER_MODES.map((m) => (
                                                <Select.Item key={m.value} value={m.value} className="select-item">
                                                    <Select.ItemText>{m.label}</Select.ItemText>
                                                </Select.Item>
                                            ))}
                                        </Select.Viewport>
                                    </Select.Content>
                                </Select.Portal>
                            </Select.Root>
                        </div>

                        {config.trigger_mode === "fixed_point_ms" && (
                            <div style={{ marginTop: 6 }}>
                                <StyledSlider
                                    label="Fixed point"
                                    value={config.fixed_crossfade_point_ms ?? 8000}
                                    min={500}
                                    max={20000}
                                    step={250}
                                    onChange={(v) => {
                                        update("fixed_crossfade_point_ms", v);
                                        update("fixed_crossfade_ms", v);
                                    }}
                                />
                            </div>
                        )}

                        {config.trigger_mode === "auto_detect_db" && (
                            <>
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Trigger dB"
                                        value={config.auto_detect_db}
                                        min={-60}
                                        max={0}
                                        step={1}
                                        onChange={(v) => update("auto_detect_db", v)}
                                        unit="dB"
                                    />
                                </div>
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Detect min"
                                        value={config.auto_detect_min_ms}
                                        min={0}
                                        max={30000}
                                        step={100}
                                        onChange={(v) => update("auto_detect_min_ms", v)}
                                    />
                                </div>
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Detect max"
                                        value={config.auto_detect_max_ms}
                                        min={500}
                                        max={45000}
                                        step={100}
                                        onChange={(v) => update("auto_detect_max_ms", v)}
                                    />
                                </div>
                            </>
                        )}

                        <div style={{ marginTop: 6 }}>
                            <StyledSlider
                                label="Min fade"
                                value={config.min_fade_time_ms}
                                min={500}
                                max={15000}
                                step={100}
                                onChange={(v) => update("min_fade_time_ms", v)}
                            />
                        </div>
                        <div style={{ marginTop: 6 }}>
                            <StyledSlider
                                label="Max fade"
                                value={config.max_fade_time_ms}
                                min={1000}
                                max={30000}
                                step={100}
                                onChange={(v) => update("max_fade_time_ms", v)}
                            />
                        </div>
                        <div style={{ marginTop: 6 }}>
                            <StyledSlider
                                label="Skip short"
                                value={config.skip_short_tracks_secs ?? 65}
                                min={0}
                                max={180}
                                step={1}
                                onChange={(v) => update("skip_short_tracks_secs", Math.round(v))}
                                unit="sec"
                            />
                        </div>

                        <div className="separator" />

                        {/* AutoDJ transition section */}
                        <div className="section-label" style={{ marginBottom: 10 }}>AutoDJ Transitions</div>
                        <div className="flex items-center gap-3 form-row">
                            <span className="form-label">Engine</span>
                            <Select.Root
                                value={autoTransition.engine}
                                onValueChange={(v) =>
                                    setAutoTransition((prev) => ({ ...prev, engine: v as AutodjTransitionEngine }))
                                }
                            >
                                <Select.Trigger className="select-trigger">
                                    <Select.Value />
                                    <ChevronDown size={12} />
                                </Select.Trigger>
                                <Select.Portal>
                                    <Select.Content className="select-content" position="popper" sideOffset={4}>
                                        <Select.Viewport>
                                            {AUTODJ_ENGINES.map((m) => (
                                                <Select.Item key={m.value} value={m.value} className="select-item">
                                                    <Select.ItemText>{m.label}</Select.ItemText>
                                                </Select.Item>
                                            ))}
                                        </Select.Viewport>
                                    </Select.Content>
                                </Select.Portal>
                            </Select.Root>
                        </div>

                        {autoTransition.engine === "mixxx_planner" && (
                            <>
                                <Checkbox
                                    checked={autoTransition.mixxx_planner_config.enabled}
                                    onCheckedChange={(v) =>
                                        setAutoTransition((prev) => ({
                                            ...prev,
                                            mixxx_planner_config: { ...prev.mixxx_planner_config, enabled: v },
                                        }))
                                    }
                                    label="Enable planner-driven transitions"
                                />
                                <div className="flex items-center gap-3 form-row" style={{ marginTop: 10 }}>
                                    <span className="form-label">Mode</span>
                                    <Select.Root
                                        value={autoTransition.mixxx_planner_config.mode}
                                        onValueChange={(v) =>
                                            setAutoTransition((prev) => ({
                                                ...prev,
                                                mixxx_planner_config: {
                                                    ...prev.mixxx_planner_config,
                                                    mode: v as AutoTransitionMode,
                                                },
                                            }))
                                        }
                                    >
                                        <Select.Trigger className="select-trigger">
                                            <Select.Value />
                                            <ChevronDown size={12} />
                                        </Select.Trigger>
                                        <Select.Portal>
                                            <Select.Content className="select-content" position="popper" sideOffset={4}>
                                                <Select.Viewport>
                                                    {AUTO_MODES.map((m) => (
                                                        <Select.Item key={m.value} value={m.value} className="select-item">
                                                            <Select.ItemText>{m.label}</Select.ItemText>
                                                        </Select.Item>
                                                    ))}
                                                </Select.Viewport>
                                            </Select.Content>
                                        </Select.Portal>
                                    </Select.Root>
                                </div>
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Transition"
                                        value={autoTransition.mixxx_planner_config.transition_time_sec}
                                        min={-15}
                                        max={30}
                                        step={1}
                                        onChange={(v) =>
                                            setAutoTransition((prev) => ({
                                                ...prev,
                                                mixxx_planner_config: {
                                                    ...prev.mixxx_planner_config,
                                                    transition_time_sec: Math.round(v),
                                                },
                                            }))
                                        }
                                        unit="s"
                                    />
                                </div>
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Min track"
                                        value={autoTransition.mixxx_planner_config.min_track_duration_ms}
                                        min={100}
                                        max={5000}
                                        step={100}
                                        onChange={(v) =>
                                            setAutoTransition((prev) => ({
                                                ...prev,
                                                mixxx_planner_config: {
                                                    ...prev.mixxx_planner_config,
                                                    min_track_duration_ms: Math.round(v),
                                                },
                                            }))
                                        }
                                    />
                                </div>
                            </>
                        )}

                        <div className="separator" />

                        {/* Preview graph */}
                        <div className="section-label" style={{ marginBottom: 8 }}>Preview</div>
                        <FadeCurveGraph
                            outCurve={config.fade_out_curve}
                            inCurve={config.fade_in_curve}
                            outTimeMs={config.fade_out_time_ms}
                            inTimeMs={config.fade_in_time_ms}
                            crossfadePointMs={config.fixed_crossfade_point_ms ?? undefined}
                            height={120}
                        />
                    </div>

                    {/* Footer */}
                    <div
                        className="flex items-center justify-between"
                        style={{
                            padding: "12px 20px",
                            borderTop: "1px solid var(--border-default)",
                            background: "var(--bg-surface)",
                            borderRadius: "0 0 var(--r-xl) var(--r-xl)",
                        }}
                    >
                        <button className="btn btn-ghost" onClick={handleReset} style={{ fontSize: 11 }}>
                            <RotateCcw size={12} />
                            Restore Defaults
                        </button>
                        <div className="flex gap-2">
                            <Dialog.Close asChild>
                                <button className="btn btn-ghost" style={{ fontSize: 11 }}>Cancel</button>
                            </Dialog.Close>
                            <button
                                className="btn btn-primary"
                                onClick={handleSave}
                                disabled={saving}
                                style={{ fontSize: 11 }}
                            >
                                {saving ? "Saving…" : "OK"}
                            </button>
                        </div>
                    </div>
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
