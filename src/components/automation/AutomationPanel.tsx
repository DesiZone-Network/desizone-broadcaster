import React, { useEffect, useState } from "react";
import {
    getDjMode, setDjMode, DjMode,
    getGapKillerConfig, setGapKillerConfig, GapKillerConfig,
} from "../../lib/bridge";
import { Bot, Zap, Radio, Sliders } from "lucide-react";

const DJ_MODE_OPTIONS: { value: DjMode; label: string; desc: string; icon: React.ReactNode }[] = [
    {
        value: "autodj",
        label: "AutoDJ",
        desc: "Station runs fully automated — rotation rules select each track",
        icon: <Bot size={18} />,
    },
    {
        value: "assisted",
        label: "Assisted",
        desc: "DJ loads queue; AutoDJ fills gaps when queue empties",
        icon: <Zap size={18} />,
    },
    {
        value: "manual",
        label: "Manual",
        desc: "DJ controls everything — AutoDJ stays silent",
        icon: <Radio size={18} />,
    },
];

export default function AutomationPanel() {
    const [djMode, setDjModeState] = useState<DjMode>("manual");
    const [gapCfg, setGapCfg] = useState<GapKillerConfig>({
        mode: "smart",
        threshold_db: -50,
        min_silence_ms: 500,
    });
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        getDjMode().then(setDjModeState).catch(() => { });
        getGapKillerConfig().then(setGapCfg).catch(() => { });
    }, []);

    const handleDjMode = async (mode: DjMode) => {
        setDjModeState(mode);
        await setDjMode(mode).catch(() => { });
    };

    const saveGap = async () => {
        setSaving(true);
        await setGapKillerConfig(gapCfg).catch(() => { });
        setSaving(false);
    };

    return (
        <div className="automation-panel">
            {/* ── DJ Mode selector ──────────────────────────────────── */}
            <section className="ap-section">
                <div className="ap-section-header">
                    <Bot size={16} />
                    <span>DJ Mode</span>
                </div>
                <div className="ap-mode-grid">
                    {DJ_MODE_OPTIONS.map((opt) => (
                        <button
                            key={opt.value}
                            className={`ap-mode-card${djMode === opt.value ? " active" : ""}`}
                            onClick={() => handleDjMode(opt.value)}
                        >
                            <span className="ap-mode-icon">{opt.icon}</span>
                            <span className="ap-mode-label">{opt.label}</span>
                            <span className="ap-mode-desc">{opt.desc}</span>
                        </button>
                    ))}
                </div>
            </section>

            {/* ── GAP Killer ───────────────────────────────────────────── */}
            <section className="ap-section">
                <div className="ap-section-header">
                    <Sliders size={16} />
                    <span>GAP Killer</span>
                </div>
                <div className="ap-gap-form">
                    <label className="ap-field">
                        <span>Mode</span>
                        <select
                            value={gapCfg.mode}
                            onChange={(e) =>
                                setGapCfg((c) => ({ ...c, mode: e.target.value as GapKillerConfig["mode"] }))
                            }
                        >
                            <option value="off">Off</option>
                            <option value="smart">Smart</option>
                            <option value="aggressive">Aggressive</option>
                        </select>
                    </label>

                    <label className="ap-field">
                        <span>Silence threshold ({gapCfg.threshold_db} dB)</span>
                        <input
                            type="range"
                            min={-80}
                            max={-20}
                            step={1}
                            value={gapCfg.threshold_db}
                            onChange={(e) =>
                                setGapCfg((c) => ({ ...c, threshold_db: parseFloat(e.target.value) }))
                            }
                        />
                    </label>

                    <label className="ap-field">
                        <span>Min silence ({gapCfg.min_silence_ms} ms)</span>
                        <input
                            type="range"
                            min={100}
                            max={3000}
                            step={50}
                            value={gapCfg.min_silence_ms}
                            onChange={(e) =>
                                setGapCfg((c) => ({ ...c, min_silence_ms: parseInt(e.target.value) }))
                            }
                        />
                    </label>

                    <button className="ap-save-btn" onClick={saveGap} disabled={saving}>
                        {saving ? "Saving…" : "Save GAP Killer"}
                    </button>
                </div>
            </section>
        </div>
    );
}
