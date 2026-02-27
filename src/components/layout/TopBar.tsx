import { useEffect, useRef, useState } from "react";
import { Wifi, WifiOff, Radio } from "lucide-react";
import {
  getMasterLevel,
  getDjMode,
  getEncoderRuntime,
  onMasterVolumeChanged,
  onDjModeChanged,
  onEncoderStatusChanged,
  onListenerCountUpdated,
  onVuMeter,
  setMasterLevel,
  VuEvent,
  type DjMode,
} from "../../lib/bridge";

interface Props {
  stationName?: string;
  isOnAir: boolean;
  streamConnected: boolean;
}

function MasterVU({ vuData }: { vuData: VuEvent | null }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    const W = canvas.width;
    const H = canvas.height;
    ctx.clearRect(0, 0, W, H);

    const leftDb = vuData?.left_db ?? -60;
    const rightDb = vuData?.right_db ?? -60;

    const drawBar = (x: number, w: number, db: number) => {
      const pct = Math.max(0, Math.min(1, (db + 60) / 60));
      const fillH = Math.round(pct * H);
      const y = H - fillH;

      // Gradient
      const grad = ctx.createLinearGradient(0, H, 0, 0);
      grad.addColorStop(0, "#16a34a");
      grad.addColorStop(0.7, "#ca8a04");
      grad.addColorStop(1, "#dc2626");
      ctx.fillStyle = grad;
      ctx.fillRect(x, y, w, fillH);

      // Dim background
      ctx.fillStyle = "#1a1a20";
      ctx.fillRect(x, 0, w, y);
    };

    const barW = Math.floor((W - 2) / 2);
    drawBar(0, barW, leftDb);
    drawBar(barW + 2, barW, rightDb);
  }, [vuData]);

  return (
    <div className="flex flex-col items-center gap-1">
      <canvas
        ref={canvasRef}
        width={28}
        height={32}
        className="vu-canvas"
        style={{ borderRadius: 3, overflow: "hidden" }}
      />
      <span className="text-xs text-muted mono">VU</span>
    </div>
  );
}

function Clock() {
  const [time, setTime] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="flex flex-col items-end">
      <span className="mono font-medium" style={{ fontSize: 18, letterSpacing: "0.06em", color: "var(--text-primary)" }}>
        {time.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
      </span>
      <span className="text-xs text-muted">
        {time.toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" })}
      </span>
    </div>
  );
}

export function TopBar({ stationName = "DesiZone Broadcaster", isOnAir, streamConnected }: Props) {
  const [masterVu, setMasterVu] = useState<VuEvent | null>(null);
  const [masterLevel, setMasterLevelState] = useState(1);
  const [djMode, setDjMode] = useState<DjMode>("manual");
  const [listenerTotal, setListenerTotal] = useState<number | null>(null);

  useEffect(() => {
    const unsub = onVuMeter((e) => {
      if (e.channel === "deck_a" || e.channel === "deck_b") {
        setMasterVu(e);
      }
    });
    return () => { unsub.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    getMasterLevel().then((level) => setMasterLevelState(level)).catch(() => {});
    const off = onMasterVolumeChanged((event) => {
      setMasterLevelState(event.level);
    });
    return () => {
      off.then((fn) => fn()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    getDjMode().then(setDjMode).catch(() => { });
    const off = onDjModeChanged((mode) => setDjMode(mode));
    const id = setInterval(() => {
      getDjMode().then(setDjMode).catch(() => { });
    }, 5000);
    return () => {
      off();
      clearInterval(id);
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    const refreshListeners = () => {
      getEncoderRuntime()
        .then((runtime) => {
          if (disposed) return;
          const total = runtime.reduce((sum, item) => sum + (item.listeners ?? 0), 0);
          setListenerTotal(total);
        })
        .catch(() => {
          if (!disposed) setListenerTotal(null);
        });
    };

    refreshListeners();
    const countTimer = setInterval(refreshListeners, 15000);
    const offStatus = onEncoderStatusChanged(() => refreshListeners());
    const offCount = onListenerCountUpdated(() => refreshListeners());

    return () => {
      disposed = true;
      clearInterval(countTimer);
      offStatus.then((fn) => fn());
      offCount.then((fn) => fn());
    };
  }, []);

  const modeLabel = djMode === "autodj" ? "AUTODJ" : djMode === "assisted" ? "ASSISTED" : "MANUAL";
  const modeStyle =
    djMode === "autodj"
      ? { background: "rgba(34,197,94,.15)", border: "1px solid rgba(34,197,94,.45)", color: "var(--green)" }
      : djMode === "assisted"
      ? { background: "var(--amber-glow)", border: "1px solid var(--amber-dim)", color: "var(--amber)" }
      : { background: "var(--bg-elevated)", border: "1px solid var(--border-strong)", color: "var(--text-muted)" };

  const handleMasterLevelChange = (next: number) => {
    const clamped = Math.max(0, Math.min(1, next));
    setMasterLevelState(clamped);
    setMasterLevel(clamped).catch(() => {});
  };

  return (
    <header
      className="flex items-center justify-between"
      style={{
        height: 52,
        padding: "0 16px",
        background: "var(--bg-surface)",
        borderBottom: "1px solid var(--border-default)",
        flexShrink: 0,
      }}
    >
      {/* Left: Logo + Station */}
      <div className="flex items-center gap-3">
        <div
          className="flex items-center justify-center"
          style={{
            width: 32,
            height: 32,
            borderRadius: "var(--r-md)",
            background: "linear-gradient(135deg, var(--amber-dim), var(--amber))",
          }}
        >
          <Radio size={16} color="#000" />
        </div>
        <div>
          <div className="font-semibold text-md" style={{ lineHeight: 1.2 }}>{stationName}</div>
          <div className="text-xs tracking-wide" style={{ color: "var(--text-muted)" }}>BROADCAST CONSOLE</div>
        </div>
      </div>

      {/* Center: Status badges */}
      <div className="flex items-center gap-3">
        {isOnAir && (
          <div className="badge badge-on-air on-air-glow">
            <div className="pulse-dot pulse-dot-red" />
            ON AIR
          </div>
        )}
        {!isOnAir && (
          <div className="badge" style={{ background: "var(--bg-elevated)", border: "1px solid var(--border-strong)", color: "var(--text-muted)" }}>
            OFF AIR
          </div>
        )}

        <div className={`badge ${streamConnected ? "badge-stream" : ""}`}
          style={!streamConnected ? { background: "var(--bg-elevated)", border: "1px solid var(--border-strong)", color: "var(--text-muted)" } : {}}>
          {streamConnected ? <Wifi size={10} /> : <WifiOff size={10} />}
          {streamConnected ? "STREAMING" : "NO STREAM"}
        </div>

        <div
          className="badge"
          style={{
            background: "rgba(6,182,212,.12)",
            border: "1px solid rgba(6,182,212,.45)",
            color: "var(--cyan)",
          }}
        >
          LISTENERS {listenerTotal ?? 0}
        </div>

        <div className="badge" style={modeStyle}>
          {modeLabel}
        </div>
      </div>

      {/* Right: VU + Clock */}
      <div className="flex items-center gap-5">
        <div className="flex flex-col items-end" style={{ minWidth: 120 }}>
          <span className="mono text-muted" style={{ fontSize: 9, letterSpacing: "0.08em" }}>MASTER</span>
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={masterLevel}
            onChange={(e) => handleMasterLevelChange(parseFloat(e.target.value))}
            style={{ width: 110, accentColor: "var(--amber)" }}
          />
          <span className="mono text-muted" style={{ fontSize: 9 }}>{Math.round(masterLevel * 100)}%</span>
        </div>
        <MasterVU vuData={masterVu} />
        <Clock />
      </div>
    </header>
  );
}
