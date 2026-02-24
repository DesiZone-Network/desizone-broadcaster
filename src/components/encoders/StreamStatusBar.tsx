import {
    EncoderConfig,
    EncoderRuntimeState,
    EncoderStatus,
} from "../../lib/bridge";
import { Wifi, WifiOff } from "lucide-react";


interface Props {
    encoders: EncoderConfig[];
    runtime: Map<number, EncoderRuntimeState>;
}

function pillClass(status: EncoderStatus): string {
    if (typeof status === "string") {
        if (status === "streaming") return "connected";
        if (status === "recording") return "connected";
        if (status === "connecting") return "retrying";
    }
    if (typeof status === "object" && "retrying" in status) return "retrying";
    return "disconnected";
}

function pillIcon(status: EncoderStatus) {
    if (typeof status === "string") {
        if (status === "streaming" || status === "recording") return <Wifi size={8} />;
    }
    return <WifiOff size={8} />;
}

function pillLabel(enc: EncoderConfig, status: EncoderStatus, listeners: number | null): string {
    const name = enc.name.length > 14 ? enc.name.slice(0, 14) + "…" : enc.name;
    if (typeof status === "string" && status === "streaming" && listeners !== null) {
        return `${name} · ${listeners}`;
    }
    if (typeof status === "string" && status === "recording") return `${name} ● REC`;
    if (typeof status === "string" && status === "connecting") return `${name} ···`;
    if (typeof status === "object" && "retrying" in status) {
        const { attempt } = status.retrying;
        return `${name} RETRY ${attempt}`;
    }
    return name;
}

export function StreamStatusBar({ encoders, runtime }: Props) {
    // Only show enabled encoders
    const visible = encoders.filter((e) => e.enabled);

    if (visible.length === 0) return null;

    return (
        <div className="stream-status-bar">
            {visible.map((enc) => {
                const rt = runtime.get(enc.id);
                const status: EncoderStatus = rt?.status ?? "disabled";
                const cls = pillClass(status);

                return (
                    <div key={enc.id} className={`stream-status-pill ${cls}`}>
                        {pillIcon(status)}
                        {pillLabel(enc, status, rt?.listeners ?? null)}
                    </div>
                );
            })}
        </div>
    );
}
