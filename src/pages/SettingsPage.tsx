import { useState, useEffect } from "react";
import { Database, CheckCircle, XCircle } from "lucide-react";
import {
    getSamDbConfig,
    getSamDbStatus,
    testSamDbConnection,
    connectSamDb,
    disconnectSamDb,
} from "../lib/bridge";
import type { SamDbStatus } from "../lib/bridge";

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

export function SettingsPage() {
    const [activeTab, setActiveTab] = useState<"sam_db" | "audio">("sam_db");
    const [form, setForm] = useState<DbForm>(DEFAULT_FORM);
    const [status, setStatus] = useState<SamDbStatus | null>(null);
    const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
    const [testing, setTesting] = useState(false);
    const [connecting, setConnecting] = useState(false);

    useEffect(() => {
        getSamDbConfig().then((cfg) => {
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
        }).catch(() => { });
        refreshStatus();
    }, []);

    const refreshStatus = () => {
        getSamDbStatus().then(setStatus).catch(() => { });
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
            {/* Tab bar */}
            <div className="tabs-list" style={{ marginBottom: 20 }}>
                {([
                    { id: "sam_db" as const, label: "SAM Database" },
                    { id: "audio" as const, label: "Audio" },
                ]).map((tab) => (
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

            {/* SAM Database tab */}
            {activeTab === "sam_db" && (
                <div style={{ maxWidth: 520 }}>
                    {/* Live status banner */}
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
                        {status?.connected
                            ? <CheckCircle size={14} style={{ color: "var(--green)", flexShrink: 0 }} />
                            : <XCircle size={14} style={{ color: "var(--red)", flexShrink: 0 }} />
                        }
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

                    {/* Connection form */}
                    <div className="section-label" style={{ marginBottom: 10 }}>Connection</div>

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
                                    onChange={(e) => setForm((f) => ({ ...f, port: parseInt(e.target.value) || 3306 }))}
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

                    {/* Auto-connect toggle */}
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

                    {/* Path translation */}
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
                            placeholder={`C:\\Music\\`}
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

                    {/* Test result banner */}
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

                    {/* Action buttons */}
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
                            <button
                                className="btn btn-danger"
                                style={{ fontSize: 11 }}
                                onClick={handleDisconnect}
                            >
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

            {/* Audio tab (placeholder) */}
            {activeTab === "audio" && (
                <div
                    style={{
                        display: "flex",
                        flexDirection: "column",
                        alignItems: "center",
                        justifyContent: "center",
                        height: 200,
                        color: "var(--text-muted)",
                        fontSize: 12,
                        gap: 8,
                    }}
                >
                    <span style={{ opacity: 0.5 }}>Audio device settings — coming soon</span>
                </div>
            )}
        </div>
    );
}
