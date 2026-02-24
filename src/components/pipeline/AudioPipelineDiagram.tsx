import { useState } from "react";

interface ChannelNode {
    id: string;
    label: string;
    x: number;
    y: number;
    active?: boolean;
    color?: string;
}

interface Props {
    onNodeClick?: (nodeId: string, channel: string) => void;
    activeChannels?: string[];
}

const SOURCES: ChannelNode[] = [
    { id: "deck_a", label: "DECK A", x: 20, y: 20, color: "#f59e0b" },
    { id: "deck_b", label: "DECK B", x: 20, y: 80, color: "#06b6d4" },
    { id: "sound_fx", label: "SFX", x: 20, y: 140, color: "#8b5cf6" },
    { id: "aux_1", label: "AUX 1", x: 20, y: 200, color: "#22c55e" },
    { id: "voice_fx", label: "VOICE", x: 20, y: 260, color: "#f97316" },
];

const DSP_STAGES = ["EQ", "AGC", "DSP"];

const W = 580;
const H = 320;
const SRC_W = 56;
const SRC_H = 28;
const NODE_W = 36;
const NODE_H = 22;
const MIXER_H = 140;

export function AudioPipelineDiagram({ onNodeClick, activeChannels = ["deck_a"] }: Props) {
    const [hovered, setHovered] = useState<string | null>(null);

    const sourceX = 10;
    const dspStartX = 90;
    const dspColW = 50;
    const mixerX = 300;
    const masterX = 380;
    const outX = 520;
    const mixerCenterY = 150;

    return (
        <div style={{ width: "100%", overflow: "auto" }}>
            <svg
                viewBox={`0 0 ${W} ${H}`}
                style={{
                    width: "100%",
                    height: "auto",
                    fontFamily: "JetBrains Mono, monospace",
                    background: "var(--bg-input)",
                    borderRadius: "var(--r-md)",
                    border: "1px solid var(--border-default)",
                }}
            >
                {/* Source channels */}
                {SOURCES.map((src) => {
                    const isActive = activeChannels.includes(src.id);
                    const cy = src.y + SRC_H / 2;

                    return (
                        <g key={src.id}>
                            {/* Source box */}
                            <rect
                                x={sourceX}
                                y={src.y}
                                width={SRC_W}
                                height={SRC_H}
                                rx={4}
                                fill={isActive ? `${src.color}20` : "#1a1a20"}
                                stroke={isActive ? src.color : "#333342"}
                                strokeWidth={1}
                                style={{ cursor: "pointer" }}
                                onClick={() => onNodeClick?.("source", src.id)}
                                onMouseEnter={() => setHovered(`src-${src.id}`)}
                                onMouseLeave={() => setHovered(null)}
                            />
                            <text
                                x={sourceX + SRC_W / 2}
                                y={src.y + SRC_H / 2 + 4}
                                textAnchor="middle"
                                fontSize={8}
                                fontWeight="600"
                                fill={isActive ? src.color : "#60607a"}
                                letterSpacing="0.08em"
                                style={{ pointerEvents: "none" }}
                            >
                                {src.label}
                            </text>

                            {/* Active dot */}
                            {isActive && (
                                <circle
                                    cx={sourceX + SRC_W - 5}
                                    cy={src.y + 5}
                                    r={3}
                                    fill={src.color}
                                />
                            )}

                            {/* Line to EQ */}
                            <line
                                x1={sourceX + SRC_W}
                                y1={cy}
                                x2={dspStartX}
                                y2={cy}
                                stroke={isActive ? src.color : "#333342"}
                                strokeWidth={1.5}
                                strokeDasharray={isActive ? "none" : "3,3"}
                            />

                            {/* DSP chain */}
                            {DSP_STAGES.map((stage, si) => {
                                const nx = dspStartX + si * dspColW;
                                const nodeId = `${src.id}-${stage.toLowerCase()}`;
                                const isHov = hovered === nodeId;

                                return (
                                    <g key={stage}>
                                        <rect
                                            x={nx}
                                            y={src.y + (SRC_H - NODE_H) / 2}
                                            width={NODE_W}
                                            height={NODE_H}
                                            rx={3}
                                            fill={isHov ? (isActive ? `${src.color}30` : "#252530") : (isActive ? `${src.color}15` : "#1a1a20")}
                                            stroke={isActive ? (isHov ? src.color : `${src.color}60`) : "#252532"}
                                            strokeWidth={1}
                                            style={{ cursor: "pointer" }}
                                            onClick={() => onNodeClick?.(stage.toLowerCase(), src.id)}
                                            onMouseEnter={() => setHovered(nodeId)}
                                            onMouseLeave={() => setHovered(null)}
                                        />
                                        <text
                                            x={nx + NODE_W / 2}
                                            y={src.y + SRC_H / 2 + 4}
                                            textAnchor="middle"
                                            fontSize={7}
                                            fontWeight="600"
                                            fill={isActive ? (isHov ? src.color : `${src.color}cc`) : "#40404a"}
                                            letterSpacing="0.06em"
                                            style={{ pointerEvents: "none" }}
                                        >
                                            {stage}
                                        </text>

                                        {/* Connector */}
                                        {si < DSP_STAGES.length - 1 && (
                                            <line
                                                x1={nx + NODE_W}
                                                y1={cy}
                                                x2={nx + dspColW}
                                                y2={cy}
                                                stroke={isActive ? `${src.color}80` : "#252532"}
                                                strokeWidth={1.2}
                                            />
                                        )}
                                    </g>
                                );
                            })}

                            {/* Line: last DSP → Mixer */}
                            <line
                                x1={dspStartX + DSP_STAGES.length * dspColW - (dspColW - NODE_W)}
                                y1={cy}
                                x2={mixerX}
                                y2={mixerCenterY}
                                stroke={isActive ? `${src.color}60` : "#1e1e28"}
                                strokeWidth={1.2}
                                strokeDasharray={isActive ? "none" : "3,3"}
                            />
                        </g>
                    );
                })}

                {/* Mixer box */}
                <rect
                    x={mixerX}
                    y={mixerCenterY - MIXER_H / 2.5}
                    width={50}
                    height={MIXER_H / 1.3}
                    rx={4}
                    fill="#1a1a25"
                    stroke="#333342"
                    strokeWidth={1.5}
                />
                <text
                    x={mixerX + 25}
                    y={mixerCenterY + 4}
                    textAnchor="middle"
                    fontSize={8}
                    fontWeight="700"
                    fill="#a0a0b8"
                    letterSpacing="0.1em"
                >
                    MIX
                </text>
                <text
                    x={mixerX + 25}
                    y={mixerCenterY + 14}
                    textAnchor="middle"
                    fontSize={7}
                    fill="#505060"
                    letterSpacing="0.04em"
                >
                    ER
                </text>

                {/* Mixer → Master chain */}
                <line
                    x1={mixerX + 50}
                    y1={mixerCenterY}
                    x2={masterX}
                    y2={mixerCenterY}
                    stroke="#333342"
                    strokeWidth={1.5}
                />

                {/* Master EQ + AGC + DSP */}
                {DSP_STAGES.map((stage, si) => {
                    const nx = masterX + si * 42;
                    const nodeId = `master-${stage.toLowerCase()}`;
                    const isHov = hovered === nodeId;
                    return (
                        <g key={`master-${stage}`}>
                            <rect
                                x={nx}
                                y={mixerCenterY - NODE_H / 2}
                                width={34}
                                height={NODE_H}
                                rx={3}
                                fill={isHov ? "#252535" : "#1e1e28"}
                                stroke={isHov ? "#6060a0" : "#2e2e42"}
                                strokeWidth={1}
                                style={{ cursor: "pointer" }}
                                onClick={() => onNodeClick?.(stage.toLowerCase(), "master")}
                                onMouseEnter={() => setHovered(nodeId)}
                                onMouseLeave={() => setHovered(null)}
                            />
                            <text
                                x={nx + 17}
                                y={mixerCenterY + 4}
                                textAnchor="middle"
                                fontSize={7}
                                fontWeight="600"
                                fill={isHov ? "#a0a0cc" : "#505060"}
                                letterSpacing="0.06em"
                                style={{ pointerEvents: "none" }}
                            >
                                {stage}
                            </text>
                            {si < DSP_STAGES.length - 1 && (
                                <line
                                    x1={nx + 34}
                                    y1={mixerCenterY}
                                    x2={nx + 42}
                                    y2={mixerCenterY}
                                    stroke="#252532"
                                    strokeWidth={1.2}
                                />
                            )}
                        </g>
                    );
                })}

                {/* Master output lines: Air Out + Encoder */}
                {/* Line to Air Out */}
                <line
                    x1={masterX + DSP_STAGES.length * 42 - 8}
                    y1={mixerCenterY}
                    x2={outX}
                    y2={mixerCenterY - 30}
                    stroke="#22c55e60"
                    strokeWidth={1.5}
                />
                <rect x={outX} y={mixerCenterY - 42} width={52} height={22} rx={3} fill="#16a34a20" stroke="#22c55e60" strokeWidth={1} />
                <text x={outX + 26} y={mixerCenterY - 27} textAnchor="middle" fontSize={7} fontWeight="700" fill="#22c55e" letterSpacing="0.08em">
                    AIR OUT
                </text>
                <circle cx={outX + 44} cy={mixerCenterY - 38} r={3} fill="#22c55e" />

                {/* Line to Encoder */}
                <line
                    x1={masterX + DSP_STAGES.length * 42 - 8}
                    y1={mixerCenterY}
                    x2={outX}
                    y2={mixerCenterY + 30}
                    stroke="#06b6d460"
                    strokeWidth={1.5}
                />
                <rect x={outX} y={mixerCenterY + 18} width={52} height={22} rx={3} fill="#06b6d420" stroke="#06b6d460" strokeWidth={1} />
                <text x={outX + 26} y={mixerCenterY + 33} textAnchor="middle" fontSize={7} fontWeight="700" fill="#06b6d4" letterSpacing="0.08em">
                    ENCODER
                </text>

                {/* Title */}
                <text x={10} y={H - 8} fontSize={8} fill="#303040" letterSpacing="0.12em" fontWeight="600">
                    AUDIO MIXER PIPELINE — CLICK NODE TO OPEN SETTINGS
                </text>
            </svg>
        </div>
    );
}
