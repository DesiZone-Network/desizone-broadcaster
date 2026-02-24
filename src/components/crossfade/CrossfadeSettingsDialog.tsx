import { useState, useEffect } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import * as Select from "@radix-ui/react-select";
import { X, ChevronDown, RotateCcw } from "lucide-react";
import {
    getCrossfadeConfig, setCrossfadeConfig,
    CrossfadeConfig, FadeCurve,
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
    fade_in_enabled: true,
    fade_in_curve: "s_curve",
    fade_in_time_ms: 10000,
    crossfade_mode: "overlap",
    auto_detect_enabled: true,
    auto_detect_db: -3,
    auto_detect_min_ms: 3000,
    auto_detect_max_ms: 10000,
    fixed_crossfade_point_ms: 8000,
};

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
    const [open, setOpen] = useState(false);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        if (open) {
            getCrossfadeConfig()
                .then(setConfig)
                .catch(() => { }); // use defaults on error
        }
    }, [open]);

    const update = <K extends keyof CrossfadeConfig>(key: K, value: CrossfadeConfig[K]) => {
        setConfig((prev) => ({ ...prev, [key]: value }));
    };

    const handleSave = async () => {
        setSaving(true);
        try {
            await setCrossfadeConfig(config);
            setOpen(false);
        } catch (e) {
            console.error("Failed to save crossfade config:", e);
        } finally {
            setSaving(false);
        }
    };

    const handleReset = () => setConfig(DEFAULT_CONFIG);

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
                            <span className="form-label">Mode</span>
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
                                                { value: "overlap", label: "Auto detect (dB level)" },
                                                { value: "segue", label: "Fixed crossfade point" },
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

                        {config.crossfade_mode === "overlap" && (
                            <div style={{ marginTop: 6 }}>
                                <StyledSlider
                                    label="Trigger at"
                                    value={config.auto_detect_db}
                                    min={-30}
                                    max={0}
                                    step={0.5}
                                    onChange={(v) => update("auto_detect_db", v)}
                                    unit="dB"
                                />
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Min fade time"
                                        value={config.auto_detect_min_ms}
                                        min={500}
                                        max={10000}
                                        step={500}
                                        onChange={(v) => update("auto_detect_min_ms", v)}
                                    />
                                </div>
                                <div style={{ marginTop: 6 }}>
                                    <StyledSlider
                                        label="Max fade time"
                                        value={config.auto_detect_max_ms}
                                        min={1000}
                                        max={20000}
                                        step={500}
                                        onChange={(v) => update("auto_detect_max_ms", v)}
                                    />
                                </div>
                            </div>
                        )}

                        {config.crossfade_mode === "segue" && (
                            <div style={{ marginTop: 6 }}>
                                <StyledSlider
                                    label="Fixed point"
                                    value={config.fixed_crossfade_point_ms ?? 8000}
                                    min={500}
                                    max={20000}
                                    step={500}
                                    onChange={(v) => update("fixed_crossfade_point_ms", v)}
                                />
                            </div>
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
