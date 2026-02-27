import { useEffect, useState } from "react";
import { CheckCircle, Database, Gamepad2, XCircle } from "lucide-react";
import {
    connectController,
    connectSamDb,
    disconnectController,
    disconnectSamDb,
    getControllerConfig,
    getControllerStatus,
    getSamDbConfig,
    getSamDbStatus,
    listControllerDevices,
    onControllerError,
    onControllerStatusChanged,
    saveControllerConfig,
    testSamDbConnection,
} from "../lib/bridge";
import type {
    ControllerConfig,
    ControllerDevice,
    ControllerStatus,
    SamDbStatus,
} from "../lib/bridge";
import {
    DEFAULT_ALBUM_ART_BASE_URL,
    getAlbumArtBaseUrl,
    setAlbumArtBaseUrl,
} from "../lib/albumArt";

interface DbForm {
    host: string;
    port: number;
    username: string;
    password: string;
    database: string;
    auto_connect: boolean;
    path_prefix_from: string;
    path_prefix_to: string;
}

const DEFAULT_FORM: DbForm = {
    host: "127.0.0.1",
    port: 3306,
    username: "sabroadcaster",
    password: "",
    database: "samdb",
    auto_connect: false,
    path_prefix_from: "",
    path_prefix_to: "",
};

const DEFAULT_CONTROLLER_CONFIG: ControllerConfig = {
    enabled: true,
    auto_connect: true,
    preferred_device_id: null,
    profile: "hercules_djcontrol_starlight",
};

export function SettingsPage() {
    const [activeTab, setActiveTab] = useState<"sam_db" | "audio" | "controllers">("sam_db");
    const [form, setForm] = useState<DbForm>(DEFAULT_FORM);
    const [albumArtBaseUrl, setAlbumArtBaseUrlState] = useState(DEFAULT_ALBUM_ART_BASE_URL);
    const [albumArtSaved, setAlbumArtSaved] = useState(false);
    const [status, setStatus] = useState<SamDbStatus | null>(null);
    const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
    const [testing, setTesting] = useState(false);
    const [connecting, setConnecting] = useState(false);

    const [controllerConfig, setControllerConfigState] = useState<ControllerConfig>(
        DEFAULT_CONTROLLER_CONFIG
    );
    const [controllerStatus, setControllerStatus] = useState<ControllerStatus | null>(null);
    const [controllerDevices, setControllerDevices] = useState<ControllerDevice[]>([]);
    const [controllerBusy, setControllerBusy] = useState(false);
    const [controllerMessage, setControllerMessage] = useState<string | null>(null);

    useEffect(() => {
        getSamDbConfig()
            .then((cfg) => {
                setForm((f) => ({
                    ...f,
                    host: cfg.host,
                    port: cfg.port,
                    username: cfg.username,
                    database: cfg.database_name,
                    auto_connect: cfg.auto_connect,
                    path_prefix_from: cfg.path_prefix_from ?? "",
                    path_prefix_to: cfg.path_prefix_to ?? "",
                }));
            })
            .catch(() => {});
        setAlbumArtBaseUrlState(getAlbumArtBaseUrl());
        refreshStatus();

        Promise.all([getControllerConfig(), getControllerStatus(), listControllerDevices()])
            .then(([cfg, st, devices]) => {
                setControllerConfigState(cfg);
                setControllerStatus(st);
                setControllerDevices(devices);
            })
            .catch(() => {});

        const unsubStatus = onControllerStatusChanged((st) => {
            setControllerStatus(st);
            setControllerMessage(st.last_error ?? null);
        });
        const unsubError = onControllerError((ev) => setControllerMessage(ev.message));
        return () => {
            unsubStatus.then((fn) => fn());
            unsubError.then((fn) => fn());
        };
    }, []);

    const refreshStatus = () => {
        getSamDbStatus().then(setStatus).catch(() => {});
    };

    const refreshControllers = async () => {
        const [devices, st] = await Promise.all([
            listControllerDevices(),
            getControllerStatus(),
        ]);
        setControllerDevices(devices);
        setControllerStatus(st);
    };

    const saveController = async (next: ControllerConfig) => {
        setControllerBusy(true);
        setControllerMessage(null);
        try {
            await saveControllerConfig(next);
            setControllerConfigState(next);
            await refreshControllers();
        } catch (e: any) {
            setControllerMessage(String(e));
        } finally {
            setControllerBusy(false);
        }
    };

    const handleTest = async () => {
        setTesting(true);
        setTestResult(null);
        try {
            const result = await testSamDbConnection({
                host: form.host,
                port: form.port,
                username: form.username,
                password: form.password,
                database: form.database,
                auto_connect: form.auto_connect,
                path_prefix_from: form.path_prefix_from || undefined,
                path_prefix_to: form.path_prefix_to || undefined,
            });
            setTestResult({
                ok: result.connected,
                msg: result.connected
                    ? `Connected to ${result.database}@${result.host}`
                    : result.error ?? "Connection failed",
            });
        } catch (e: any) {
            setTestResult({ ok: false, msg: String(e) });
        } finally {
            setTesting(false);
        }
    };

    const handleConnect = async () => {
        setConnecting(true);
        setTestResult(null);
        try {
            const result = await connectSamDb({
                host: form.host,
                port: form.port,
                username: form.username,
                password: form.password,
                database: form.database,
                auto_connect: form.auto_connect,
                path_prefix_from: form.path_prefix_from || undefined,
                path_prefix_to: form.path_prefix_to || undefined,
            });
            setStatus(result);
            setForm((f) => ({ ...f, auto_connect: true }));
            if (!result.connected) {
                setTestResult({ ok: false, msg: result.error ?? "Failed to connect" });
            }
        } catch (e: any) {
            setTestResult({ ok: false, msg: String(e) });
        } finally {
            setConnecting(false);
        }
    };

    const handleDisconnect = async () => {
        try {
            await disconnectSamDb();
            refreshStatus();
        } catch (e) {
            console.error(e);
        }
    };

    return (
        <div style={{ height: "100%", overflow: "auto", padding: "20px 24px" }}>
            <div className="tabs-list" style={{ marginBottom: 20 }}>
                {[
                    { id: "sam_db" as const, label: "SAM Database" },
                    { id: "audio" as const, label: "Audio" },
                    { id: "controllers" as const, label: "Controllers" },
                ].map((tab) => (
                    <button
                        key={tab.id}
                        className="tab-trigger"
                        data-state={activeTab === tab.id ? "active" : "inactive"}
                        onClick={() => setActiveTab(tab.id)}
                    >
                        {tab.label}
                    </button>
                ))}
            </div>

            {activeTab === "sam_db" && (
                <div style={{ maxWidth: 520 }}>
                    <div
                        style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 8,
                            padding: "8px 12px",
                            borderRadius: "var(--r-md)",
                            background: status?.connected ? "rgba(16,185,129,.12)" : "rgba(239,68,68,.10)",
                            border: `1px solid ${status?.connected ? "rgba(16,185,129,.3)" : "rgba(239,68,68,.25)"}`,
                            marginBottom: 20,
                            fontSize: 12,
                        }}
                    >
                        {status?.connected ? (
                            <CheckCircle size={14} style={{ color: "var(--green)", flexShrink: 0 }} />
                        ) : (
                            <XCircle size={14} style={{ color: "var(--red)", flexShrink: 0 }} />
                        )}
                        <span style={{ color: status?.connected ? "var(--green)" : "var(--red)" }}>
                            {status?.connected
                                ? `Connected — ${status.database}@${status.host}`
                                : status?.error ?? "Not connected"}
                        </span>
                        <button
                            className="btn btn-ghost"
                            style={{ marginLeft: "auto", fontSize: 9, padding: "2px 7px" }}
                            onClick={refreshStatus}
                        >
                            Refresh
                        </button>
                    </div>

                    <div className="section-label" style={{ marginBottom: 10 }}>
                        Connection
                    </div>

                    <div style={{ display: "flex", gap: 8 }}>
                        <div style={{ flex: 1 }}>
                            <div className="form-row">
                                <span className="form-label">Host</span>
                                <input
                                    type="text"
                                    className="input"
                                    value={form.host}
                                    onChange={(e) => setForm((f) => ({ ...f, host: e.target.value }))}
                                />
                            </div>
                        </div>
                        <div style={{ width: 90 }}>
                            <div className="form-row">
                                <span className="form-label">Port</span>
                                <input
                                    type="number"
                                    className="input"
                                    value={form.port}
                                    onChange={(e) =>
                                        setForm((f) => ({ ...f, port: parseInt(e.target.value, 10) || 3306 }))
                                    }
                                />
                            </div>
                        </div>
                    </div>

                    <div className="form-row">
                        <span className="form-label">Username</span>
                        <input
                            type="text"
                            className="input"
                            value={form.username}
                            onChange={(e) => setForm((f) => ({ ...f, username: e.target.value }))}
                        />
                    </div>

                    <div className="form-row">
                        <span className="form-label">Password</span>
                        <input
                            type="password"
                            className="input"
                            value={form.password}
                            onChange={(e) => setForm((f) => ({ ...f, password: e.target.value }))}
                            placeholder="••••••••"
                        />
                    </div>

                    <div className="form-row">
                        <span className="form-label">Database</span>
                        <input
                            type="text"
                            className="input"
                            value={form.database}
                            onChange={(e) => setForm((f) => ({ ...f, database: e.target.value }))}
                        />
                    </div>

                    <div
                        className="form-row"
                        style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
                    >
                        <span className="form-label">Auto-Connect on startup</span>
                        <button
                            className="btn btn-ghost"
                            style={{
                                fontSize: 10,
                                background: form.auto_connect ? "rgba(16,185,129,.15)" : "var(--bg-elevated)",
                                borderColor: form.auto_connect ? "rgba(16,185,129,.4)" : "var(--border-default)",
                                color: form.auto_connect ? "var(--green)" : "var(--text-muted)",
                                padding: "3px 10px",
                            }}
                            onClick={() => setForm((f) => ({ ...f, auto_connect: !f.auto_connect }))}
                        >
                            {form.auto_connect ? "ON" : "OFF"}
                        </button>
                    </div>

                    <div className="section-label" style={{ marginTop: 20, marginBottom: 10 }}>
                        Windows Path Translation{" "}
                        <span style={{ fontWeight: 400, color: "var(--text-dim)", fontSize: 10 }}>(optional)</span>
                    </div>

                    <div className="form-row">
                        <span className="form-label">Prefix From (Windows root)</span>
                        <input
                            type="text"
                            className="input"
                            value={form.path_prefix_from}
                            placeholder={"C:\\Music\\"}
                            onChange={(e) => setForm((f) => ({ ...f, path_prefix_from: e.target.value }))}
                        />
                    </div>

                    <div className="form-row">
                        <span className="form-label">Prefix To (local mount)</span>
                        <input
                            type="text"
                            className="input"
                            value={form.path_prefix_to}
                            placeholder="/Volumes/Music/"
                            onChange={(e) => setForm((f) => ({ ...f, path_prefix_to: e.target.value }))}
                        />
                    </div>

                    {testResult && (
                        <div
                            style={{
                                marginTop: 12,
                                padding: "7px 12px",
                                borderRadius: "var(--r-md)",
                                background: testResult.ok ? "rgba(16,185,129,.12)" : "rgba(239,68,68,.10)",
                                border: `1px solid ${testResult.ok ? "rgba(16,185,129,.3)" : "rgba(239,68,68,.25)"}`,
                                fontSize: 11,
                                color: testResult.ok ? "var(--green)" : "var(--red)",
                                display: "flex",
                                alignItems: "center",
                                gap: 6,
                            }}
                        >
                            {testResult.ok ? <CheckCircle size={13} /> : <XCircle size={13} />}
                            {testResult.msg}
                        </div>
                    )}

                    <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
                        <button
                            className="btn btn-ghost"
                            style={{ fontSize: 11 }}
                            onClick={handleTest}
                            disabled={testing}
                        >
                            <Database size={12} />
                            {testing ? "Testing…" : "Test Connection"}
                        </button>

                        {status?.connected ? (
                            <button className="btn btn-danger" style={{ fontSize: 11 }} onClick={handleDisconnect}>
                                Disconnect
                            </button>
                        ) : (
                            <button
                                className="btn btn-primary"
                                style={{ fontSize: 11 }}
                                onClick={handleConnect}
                                disabled={connecting}
                            >
                                {connecting ? "Connecting…" : "Connect"}
                            </button>
                        )}
                    </div>
                </div>
            )}

            {activeTab === "audio" && (
                <div style={{ maxWidth: 700 }}>
                    <div className="section-label" style={{ marginBottom: 10 }}>
                        Album Art
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 10 }}>
                        Base URL used when SAM `picture` stores only a file name.
                    </div>
                    <div className="form-row">
                        <span className="form-label">Album Art Base URL</span>
                        <input
                            type="text"
                            className="input"
                            value={albumArtBaseUrl}
                            onChange={(e) => {
                                setAlbumArtSaved(false);
                                setAlbumArtBaseUrlState(e.target.value);
                            }}
                            placeholder={DEFAULT_ALBUM_ART_BASE_URL}
                        />
                    </div>
                    <div className="flex items-center gap-2" style={{ marginTop: 10 }}>
                        <button
                            className="btn btn-ghost"
                            style={{ fontSize: 11 }}
                            onClick={() => {
                                setAlbumArtSaved(false);
                                setAlbumArtBaseUrlState(DEFAULT_ALBUM_ART_BASE_URL);
                            }}
                        >
                            Reset Default
                        </button>
                        <button
                            className="btn btn-primary"
                            style={{ fontSize: 11 }}
                            onClick={() => {
                                const saved = setAlbumArtBaseUrl(albumArtBaseUrl);
                                setAlbumArtBaseUrlState(saved);
                                setAlbumArtSaved(true);
                                window.setTimeout(() => setAlbumArtSaved(false), 2000);
                            }}
                        >
                            Save
                        </button>
                        {albumArtSaved && (
                            <span style={{ fontSize: 11, color: "var(--green)" }}>
                                Saved
                            </span>
                        )}
                    </div>
                    <div style={{ marginTop: 8, fontSize: 10, color: "var(--text-dim)" }}>
                        Example: {DEFAULT_ALBUM_ART_BASE_URL}
                    </div>
                </div>
            )}

            {activeTab === "controllers" && (
                <div style={{ maxWidth: 560 }}>
                    <div
                        style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 8,
                            padding: "8px 12px",
                            borderRadius: "var(--r-md)",
                            background: controllerStatus?.connected ? "rgba(16,185,129,.12)" : "rgba(148,163,184,.08)",
                            border: `1px solid ${controllerStatus?.connected ? "rgba(16,185,129,.3)" : "var(--border-default)"}`,
                            marginBottom: 16,
                            fontSize: 12,
                        }}
                    >
                        <Gamepad2
                            size={14}
                            style={{ color: controllerStatus?.connected ? "var(--green)" : "var(--text-muted)" }}
                        />
                        <span style={{ color: controllerStatus?.connected ? "var(--green)" : "var(--text-muted)" }}>
                            {controllerStatus?.connected
                                ? `Connected — ${controllerStatus.active_device_name ?? "Unknown"}`
                                : controllerConfig.enabled
                                    ? "Controller enabled, waiting for device"
                                    : "Controller disabled"}
                        </span>
                        <button
                            className="btn btn-ghost"
                            style={{ marginLeft: "auto", fontSize: 9, padding: "2px 7px" }}
                            onClick={() => refreshControllers().catch(() => {})}
                        >
                            Refresh
                        </button>
                    </div>

                    <div
                        className="form-row"
                        style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
                    >
                        <span className="form-label">Enable DJ Controller</span>
                        <button
                            className="btn btn-ghost"
                            style={{
                                fontSize: 10,
                                background: controllerConfig.enabled ? "rgba(16,185,129,.15)" : "var(--bg-elevated)",
                                borderColor: controllerConfig.enabled ? "rgba(16,185,129,.4)" : "var(--border-default)",
                                color: controllerConfig.enabled ? "var(--green)" : "var(--text-muted)",
                                padding: "3px 10px",
                            }}
                            disabled={controllerBusy}
                            onClick={() =>
                                saveController({ ...controllerConfig, enabled: !controllerConfig.enabled }).catch(
                                    () => {}
                                )
                            }
                        >
                            {controllerConfig.enabled ? "ON" : "OFF"}
                        </button>
                    </div>

                    <div
                        className="form-row"
                        style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
                    >
                        <span className="form-label">Auto-Connect on startup</span>
                        <button
                            className="btn btn-ghost"
                            style={{
                                fontSize: 10,
                                background: controllerConfig.auto_connect ? "rgba(16,185,129,.15)" : "var(--bg-elevated)",
                                borderColor: controllerConfig.auto_connect ? "rgba(16,185,129,.4)" : "var(--border-default)",
                                color: controllerConfig.auto_connect ? "var(--green)" : "var(--text-muted)",
                                padding: "3px 10px",
                            }}
                            disabled={controllerBusy}
                            onClick={() =>
                                saveController({
                                    ...controllerConfig,
                                    auto_connect: !controllerConfig.auto_connect,
                                }).catch(() => {})
                            }
                        >
                            {controllerConfig.auto_connect ? "ON" : "OFF"}
                        </button>
                    </div>

                    <div className="form-row">
                        <span className="form-label">Preferred Device</span>
                        <select
                            className="input"
                            value={controllerConfig.preferred_device_id ?? ""}
                            disabled={controllerBusy}
                            onChange={(e) => {
                                const value = e.target.value || null;
                                saveController({
                                    ...controllerConfig,
                                    preferred_device_id: value,
                                }).catch(() => {});
                            }}
                        >
                            <option value="">Auto-select (Starlight first)</option>
                            {controllerDevices.map((device) => (
                                <option key={device.id} value={device.id}>
                                    {device.name}
                                </option>
                            ))}
                        </select>
                    </div>

                    <div style={{ display: "flex", gap: 8, marginTop: 14 }}>
                        <button
                            className="btn btn-primary"
                            style={{ fontSize: 11 }}
                            disabled={controllerBusy || !controllerConfig.enabled}
                            onClick={async () => {
                                setControllerBusy(true);
                                setControllerMessage(null);
                                try {
                                    const st = await connectController(
                                        controllerConfig.preferred_device_id ?? null
                                    );
                                    setControllerStatus(st);
                                    await refreshControllers();
                                } catch (e: any) {
                                    setControllerMessage(String(e));
                                } finally {
                                    setControllerBusy(false);
                                }
                            }}
                        >
                            Connect
                        </button>
                        <button
                            className="btn btn-danger"
                            style={{ fontSize: 11 }}
                            disabled={controllerBusy}
                            onClick={async () => {
                                setControllerBusy(true);
                                setControllerMessage(null);
                                try {
                                    const st = await disconnectController();
                                    setControllerStatus(st);
                                    await refreshControllers();
                                } catch (e: any) {
                                    setControllerMessage(String(e));
                                } finally {
                                    setControllerBusy(false);
                                }
                            }}
                        >
                            Disconnect
                        </button>
                    </div>

                    {controllerMessage && (
                        <div
                            style={{
                                marginTop: 12,
                                padding: "7px 12px",
                                borderRadius: "var(--r-md)",
                                background: "rgba(239,68,68,.10)",
                                border: "1px solid rgba(239,68,68,.25)",
                                fontSize: 11,
                                color: "var(--red)",
                            }}
                        >
                            {controllerMessage}
                        </div>
                    )}

                    <div
                        style={{
                            marginTop: 16,
                            fontSize: 11,
                            color: "var(--text-muted)",
                            background: "var(--bg-elevated)",
                            border: "1px solid var(--border-default)",
                            borderRadius: "var(--r-md)",
                            padding: "10px 12px",
                        }}
                    >
                        MVP limitations: no LED feedback and no split cue/master hardware audio routing yet.
                    </div>
                </div>
            )}
        </div>
    );
}
