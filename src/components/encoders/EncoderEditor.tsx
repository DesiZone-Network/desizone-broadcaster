import { useState } from "react";
import {
    EncoderConfig,
    EncoderCodec,
    OutputType,
    FileRotation,
    saveEncoder,
    testEncoderConnection,
} from "../../lib/bridge";
import { X, CheckCircle2, XCircle, Loader2, Folder } from "lucide-react";

// ── Default config factory ─────────────────────────────────────────────────────

let nextTempId = -1;

function makeDefault(): EncoderConfig {
    return {
        id: nextTempId--,
        name: "New Encoder",
        enabled: true,

        codec: "mp3",
        bitrate_kbps: 128,
        sample_rate: 44100,
        channels: 2,
        quality: null,

        output_type: "icecast",

        server_host: "",
        server_port: 8000,
        server_username: "source",
        server_password: "",
        mount_point: "/stream",
        icecast_version: "v2",
        shoutcast_version: "v2",
        shoutcast_sid: 1,
        stream_name: "DesiZone",
        stream_genre: "Bollywood",
        stream_url: null,
        stream_description: null,
        is_public: false,

        file_output_path: "./recordings",
        file_rotation: "hourly",
        file_max_size_mb: 200,
        file_name_template: "desizone_{datetime}.wav",

        send_metadata: true,
        icy_metadata_interval: 16000,
        metadata_caption_template: "$combine$",
        metadata_url_append: null,

        reconnect_delay_secs: 10,
        max_reconnect_attempts: 0,
    };
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function FormField({
    label,
    children,
    half,
}: {
    label: string;
    children: React.ReactNode;
    half?: boolean;
}) {
    return (
        <div style={{ display: "flex", flexDirection: "column", gap: 4, flex: half ? "0 0 calc(50% - 6px)" : "1 1 100%" }}>
            <label style={{ fontSize: 10, fontWeight: 600, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--text-muted)" }}>
                {label}
            </label>
            {children}
        </div>
    );
}

function Toggle({ value, onChange, label }: { value: boolean; onChange: (v: boolean) => void; label: string }) {
    return (
        <div className="toggle-wrap" onClick={() => onChange(!value)}>
            <div className={`toggle-track ${value ? "on" : ""}`}>
                <div className="toggle-thumb" />
            </div>
            <span className="toggle-label">{label}</span>
        </div>
    );
}

// ── Tab components ─────────────────────────────────────────────────────────────

type Tab = "general" | "connection" | "codec" | "advanced";

function TabGeneral({ enc, set }: { enc: EncoderConfig; set: <K extends keyof EncoderConfig>(k: K, v: EncoderConfig[K]) => void }) {
    return (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
            <FormField label="Name">
                <input className="input" value={enc.name} onChange={(e) => set("name", e.target.value)} placeholder="e.g. Icecast Main" />
            </FormField>
            <Toggle value={enc.enabled} onChange={(v) => set("enabled", v)} label="Enabled on startup" />

            <FormField label="Output Type">
                <div className="type-pill-group">
                    {(["icecast", "shoutcast", "file"] as OutputType[]).map((t) => (
                        <button
                            key={t}
                            className={`type-pill ${enc.output_type === t ? "active" : ""}`}
                            onClick={() => set("output_type", t)}
                        >
                            {t === "file" ? "📁 File" : t === "icecast" ? "🎙 Icecast" : "📻 Shoutcast"}
                        </button>
                    ))}
                </div>
            </FormField>
        </div>
    );
}

function TabConnection({
    enc,
    set,
    onTest,
    testState,
    testError,
}: {
    enc: EncoderConfig;
    set: <K extends keyof EncoderConfig>(k: K, v: EncoderConfig[K]) => void;
    onTest: () => void;
    testState: "idle" | "testing" | "ok" | "fail";
    testError: string | null;
}) {
    if (enc.output_type === "file") {
        return (
            <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
                <FormField label="Output Directory">
                    <div style={{ display: "flex", gap: 6 }}>
                        <input
                            className="input"
                            value={enc.file_output_path ?? ""}
                            onChange={(e) => set("file_output_path", e.target.value)}
                            placeholder="./recordings"
                            style={{ flex: 1 }}
                        />
                        <button className="btn btn-ghost btn-icon" title="Browse" style={{ flexShrink: 0 }}>
                            <Folder size={14} />
                        </button>
                    </div>
                </FormField>

                <FormField label="File Name Template" half>
                    <input
                        className="input"
                        value={enc.file_name_template}
                        onChange={(e) => set("file_name_template", e.target.value)}
                        placeholder="{station}_{datetime}.wav"
                    />
                </FormField>
                <FormField label="Rotation" half>
                    <select className="input" value={enc.file_rotation} onChange={(e) => set("file_rotation", e.target.value as FileRotation)}>
                        <option value="none">None</option>
                        <option value="hourly">Hourly</option>
                        <option value="daily">Daily</option>
                        <option value="by_size">By Size</option>
                    </select>
                </FormField>

                {enc.file_rotation === "by_size" && (
                    <FormField label="Max File Size (MB)" half>
                        <input
                            className="input"
                            type="number"
                            value={enc.file_max_size_mb}
                            onChange={(e) => set("file_max_size_mb", Number(e.target.value))}
                        />
                    </FormField>
                )}
            </div>
        );
    }

    return (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
            {enc.output_type === "icecast" && (
                <FormField label="Icecast Version" half>
                    <select
                        className="input"
                        value={enc.icecast_version}
                        onChange={(e) => set("icecast_version", e.target.value as EncoderConfig["icecast_version"])}
                    >
                        <option value="v1">Icecast 1</option>
                        <option value="v2">Icecast 2</option>
                    </select>
                </FormField>
            )}
            {enc.output_type === "shoutcast" && (
                <>
                    <FormField label="Shoutcast Version" half>
                        <select
                            className="input"
                            value={enc.shoutcast_version}
                            onChange={(e) => set("shoutcast_version", e.target.value as EncoderConfig["shoutcast_version"])}
                        >
                            <option value="v1">Shoutcast v1</option>
                            <option value="v2">Shoutcast v2</option>
                        </select>
                    </FormField>
                    {enc.shoutcast_version === "v2" && (
                        <FormField label="SID" half>
                            <input
                                className="input"
                                type="number"
                                value={enc.shoutcast_sid}
                                onChange={(e) => set("shoutcast_sid", Number(e.target.value))}
                                min={1}
                            />
                        </FormField>
                    )}
                </>
            )}
            <FormField label="Server Host" half>
                <input
                    className="input"
                    value={enc.server_host ?? ""}
                    onChange={(e) => set("server_host", e.target.value)}
                    placeholder="localhost"
                />
            </FormField>
            <FormField label="Port" half>
                <input
                    className="input"
                    type="number"
                    value={enc.server_port ?? 8000}
                    onChange={(e) => set("server_port", Number(e.target.value))}
                />
            </FormField>

            <FormField label="Username" half>
                <input
                    className="input"
                    value={enc.server_username ?? ""}
                    onChange={(e) => set("server_username", e.target.value)}
                    placeholder={enc.output_type === "icecast" ? "source" : "encoder"}
                />
            </FormField>
            <FormField label="Password">
                <input
                    className="input"
                    type="password"
                    value={enc.server_password ?? ""}
                    onChange={(e) => set("server_password", e.target.value)}
                    placeholder="hackme"
                />
            </FormField>

            {(enc.output_type === "icecast" || enc.output_type === "shoutcast") && (
                <>
                    {enc.output_type === "icecast" && (
                        <FormField label="Mount Point" half>
                            <input
                                className="input"
                                value={enc.mount_point ?? ""}
                                onChange={(e) => set("mount_point", e.target.value)}
                                placeholder="/stream"
                            />
                        </FormField>
                    )}
                    <FormField label="Stream Name" half>
                        <input
                            className="input"
                            value={enc.stream_name ?? ""}
                            onChange={(e) => set("stream_name", e.target.value)}
                            placeholder="DesiZone"
                        />
                    </FormField>
                    <FormField label="Genre" half>
                        <input
                            className="input"
                            value={enc.stream_genre ?? ""}
                            onChange={(e) => set("stream_genre", e.target.value)}
                            placeholder="Bollywood"
                        />
                    </FormField>
                    <FormField label="Stream URL" half>
                        <input
                            className="input"
                            value={enc.stream_url ?? ""}
                            onChange={(e) => set("stream_url", e.target.value || null)}
                            placeholder="https://listen.example.com/stream"
                        />
                    </FormField>
                    <Toggle value={enc.is_public} onChange={(v) => set("is_public", v)} label="Public (listed in Icecast directory)" />
                </>
            )}

            {/* Test Connection button */}
            <div style={{ marginTop: 4, display: "flex", alignItems: "center", gap: 8 }}>
                <button
                    className={`btn-test ${testState === "ok" ? "success" : testState === "fail" ? "failure" : ""}`}
                    onClick={onTest}
                    disabled={testState === "testing"}
                >
                    {testState === "testing" ? <Loader2 size={11} className="spin" /> : null}
                    {testState === "ok" ? <CheckCircle2 size={11} /> : null}
                    {testState === "fail" ? <XCircle size={11} /> : null}
                    {testState === "idle" ? "Test Connection" : null}
                    {testState === "testing" ? "Testing…" : testState === "ok" ? "Connected" : testState === "fail" ? "Failed" : ""}
                </button>
            </div>
            {testError && (
                <div style={{ marginTop: 6, fontSize: 11, color: "var(--red)", maxWidth: "100%" }}>
                    {testError}
                </div>
            )}
        </div>
    );
}

function TabCodec({ enc, set }: { enc: EncoderConfig; set: <K extends keyof EncoderConfig>(k: K, v: EncoderConfig[K]) => void }) {
    const codecs: EncoderCodec[] = enc.output_type === "file"
        ? ["wav", "flac", "mp3", "aac"]
        : ["mp3", "aac", "ogg"];

    return (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
            <FormField label="Codec">
                <div className="type-pill-group">
                    {codecs.map((c) => (
                        <button
                            key={c}
                            className={`type-pill ${enc.codec === c ? "active" : ""}`}
                            onClick={() => set("codec", c)}
                        >
                            {c.toUpperCase()}
                        </button>
                    ))}
                </div>
            </FormField>

            {enc.output_type !== "file" || ["mp3", "aac", "ogg"].includes(enc.codec) ? (
                <FormField label="Bitrate (kbps)" half>
                    <select
                        className="input"
                        value={enc.bitrate_kbps ?? 128}
                        onChange={(e) => set("bitrate_kbps", Number(e.target.value))}
                    >
                        {[32, 48, 64, 96, 128, 160, 192, 256, 320].map((b) => (
                            <option key={b} value={b}>{b} kbps</option>
                        ))}
                    </select>
                </FormField>
            ) : null}

            <FormField label="Sample Rate" half>
                <select
                    className="input"
                    value={enc.sample_rate}
                    onChange={(e) => set("sample_rate", Number(e.target.value))}
                >
                    {[22050, 44100, 48000].map((r) => (
                        <option key={r} value={r}>{r} Hz</option>
                    ))}
                </select>
            </FormField>

            <FormField label="Channels" half>
                <select
                    className="input"
                    value={enc.channels}
                    onChange={(e) => set("channels", Number(e.target.value))}
                >
                    <option value={1}>Mono</option>
                    <option value={2}>Stereo</option>
                </select>
            </FormField>

            <Toggle value={enc.send_metadata} onChange={(v) => set("send_metadata", v)} label="Send ICY metadata (track titles)" />
        </div>
    );
}

function TabAdvanced({ enc, set }: { enc: EncoderConfig; set: <K extends keyof EncoderConfig>(k: K, v: EncoderConfig[K]) => void }) {
    return (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
            <FormField label="Reconnect Delay (seconds)" half>
                <input
                    className="input"
                    type="number"
                    value={enc.reconnect_delay_secs}
                    onChange={(e) => set("reconnect_delay_secs", Number(e.target.value))}
                    min={1}
                    max={300}
                />
            </FormField>
            <FormField label="Max Reconnect Attempts" half>
                <input
                    className="input"
                    type="number"
                    value={enc.max_reconnect_attempts}
                    onChange={(e) => set("max_reconnect_attempts", Number(e.target.value))}
                    min={0}
                />
                <span style={{ fontSize: 10, color: "var(--text-muted)" }}>0 = unlimited</span>
            </FormField>
            {enc.send_metadata && (
                <FormField label="ICY Metadata Interval (bytes)" half>
                    <input
                        className="input"
                        type="number"
                        value={enc.icy_metadata_interval}
                        onChange={(e) => set("icy_metadata_interval", Number(e.target.value))}
                        min={4096}
                        step={4096}
                    />
                </FormField>
            )}
            {enc.send_metadata && (
                <FormField label="Caption Template">
                    <input
                        className="input"
                        value={enc.metadata_caption_template ?? ""}
                        onChange={(e) => set("metadata_caption_template", e.target.value || null)}
                        placeholder="$combine$"
                    />
                </FormField>
            )}
            {enc.output_type === "icecast" && enc.send_metadata && (
                <FormField label="URL Append Template">
                    <input
                        className="input"
                        value={enc.metadata_url_append ?? ""}
                        onChange={(e) => set("metadata_url_append", e.target.value || null)}
                        placeholder="&artist=$artist$&title=$title$"
                    />
                </FormField>
            )}
            <FormField label="Stream Description">
                <textarea
                    className="input"
                    rows={2}
                    value={enc.stream_description ?? ""}
                    onChange={(e) => set("stream_description", e.target.value || null)}
                    placeholder="Hindi music 24/7"
                    style={{ resize: "vertical" }}
                />
            </FormField>
        </div>
    );
}

// ── Main EncoderEditor dialog ─────────────────────────────────────────────────

interface Props {
    initial: EncoderConfig | null; // null = create new
    onClose: () => void;
    onSaved: (updated: EncoderConfig) => void;
}

export function EncoderEditor({ initial, onClose, onSaved }: Props) {
    const [enc, setEncState] = useState<EncoderConfig>(() => initial ?? makeDefault());
    const [tab, setTab] = useState<Tab>("general");
    const [saving, setSaving] = useState(false);
    const [testState, setTestState] = useState<"idle" | "testing" | "ok" | "fail">("idle");
    const [error, setError] = useState<string | null>(null);
    const [testError, setTestError] = useState<string | null>(null);

    const set = <K extends keyof EncoderConfig>(key: K, value: EncoderConfig[K]) =>
        setEncState((prev) => ({ ...prev, [key]: value }));

    const handleSave = async () => {
        setSaving(true);
        setError(null);
        try {
            const id = await saveEncoder(enc);
            onSaved({ ...enc, id });
        } catch (e: any) {
            setError(String(e));
        } finally {
            setSaving(false);
        }
    };

    const handleTest = async () => {
        setTestError(null);
        setTestState("testing");
        try {
            const ok = await testEncoderConnection(enc.id);
            setTestState(ok ? "ok" : "fail");
            if (!ok) {
                setTestError("Connection test returned failure.");
            }
        } catch (e: any) {
            setTestState("fail");
            setTestError(String(e));
        }
        setTimeout(() => setTestState("idle"), 4000);
    };

    const TABS: { id: Tab; label: string }[] = [
        { id: "general", label: "General" },
        { id: "connection", label: enc.output_type === "file" ? "File Output" : "Connection" },
        { id: "codec", label: "Codec" },
        { id: "advanced", label: "Advanced" },
    ];

    return (
        <div
            className="modal-backdrop"
            onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
        >
            <div className="modal encoder-editor" onClick={(e) => e.stopPropagation()}>
                {/* Modal header */}
                <div className="modal-header">
                    <span style={{ fontWeight: 600, fontSize: 14, color: "var(--text-primary)" }}>
                        {initial ? `Edit Encoder — ${initial.name}` : "Add Encoder"}
                    </span>
                    <button className="btn btn-ghost btn-icon" onClick={onClose}>
                        <X size={14} />
                    </button>
                </div>

                {/* Tabs */}
                <div className="encoder-editor-tabs">
                    {TABS.map((t) => (
                        <button
                            key={t.id}
                            className={`tab-btn ${tab === t.id ? "active" : ""}`}
                            onClick={() => setTab(t.id)}
                        >
                            {t.label}
                        </button>
                    ))}
                </div>

                {/* Body */}
                <div className="encoder-editor-body">
                    {tab === "general" && <TabGeneral enc={enc} set={set} />}
                    {tab === "connection" && (
                        <TabConnection
                            enc={enc}
                            set={set}
                            onTest={handleTest}
                            testState={testState}
                            testError={testError}
                        />
                    )}
                    {tab === "codec" && <TabCodec enc={enc} set={set} />}
                    {tab === "advanced" && <TabAdvanced enc={enc} set={set} />}
                </div>

                {/* Footer */}
                <div
                    style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        padding: "12px 16px",
                        borderTop: "1px solid var(--border-default)",
                    }}
                >
                    {error ? (
                        <span style={{ fontSize: 11, color: "var(--red)" }}>✕ {error}</span>
                    ) : (
                        <span />
                    )}
                    <div style={{ display: "flex", gap: 8 }}>
                        <button className="btn btn-ghost" onClick={onClose}>
                            Cancel
                        </button>
                        <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
                            {saving ? "Saving…" : "Save Encoder"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
