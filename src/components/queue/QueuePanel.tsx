import { useState, useEffect, useCallback } from "react";
import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    DragEndEvent,
} from "@dnd-kit/core";
import {
    SortableContext,
    sortableKeyboardCoordinates,
    useSortable,
    verticalListSortingStrategy,
    arrayMove,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripVertical, Trash2, Music2, Clock, Plus } from "lucide-react";
import {
    enqueueNextClockwheelTrack,
    getQueue,
    onDeckStateChanged,
    removeFromQueue,
    QueueEntry,
    SamSong,
} from "../../lib/bridge";
import { serializeSongDragPayload } from "../../lib/songDrag";

interface QueueItem extends QueueEntry {
    song?: SamSong;
}

function formatDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
}

function SortableQueueRow({
    item,
    index,
    isNowPlaying,
    onRemove,
}: {
    item: QueueItem;
    index: number;
    isNowPlaying: boolean;
    onRemove: (id: number) => void;
}) {
    const { attributes, listeners, setNodeRef, transform, transition, isDragging } =
        useSortable({ id: item.id ?? index });

    return (
        <div
            ref={setNodeRef}
            className={`list-row ${isNowPlaying ? "now-playing" : ""}`}
            style={{
                transform: CSS.Transform.toString(transform),
                transition,
                opacity: isDragging ? 0.5 : 1,
                cursor: "default",
            }}
            draggable={Boolean(item.song?.filename)}
            onDragStart={(e) => {
                if (!item.song?.filename) return;
                e.dataTransfer.setData("text/plain", serializeSongDragPayload(item.song, "queue"));
                e.dataTransfer.effectAllowed = "copy";
            }}
        >
            <div className="drag-handle" {...attributes} {...listeners}>
                <GripVertical size={12} />
            </div>

            <span
                className="mono"
                style={{
                    fontSize: 10,
                    color: isNowPlaying ? "var(--amber)" : "var(--text-muted)",
                    minWidth: 20,
                    textAlign: "right",
                }}
            >
                {isNowPlaying ? "▶" : index + 1}
            </span>

            <Music2
                size={12}
                style={{ color: isNowPlaying ? "var(--amber)" : "var(--text-muted)", flexShrink: 0 }}
            />

            <div className="flex-1 min-w-0">
                <div
                    className="font-medium"
                    style={{
                        fontSize: 12,
                        color: isNowPlaying ? "var(--amber)" : "var(--text-primary)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                    }}
                >
                    {item.song?.title ?? `Song #${item.song_id}`}
                </div>
                <div
                    className="text-muted"
                    style={{ fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                >
                    {item.song?.artist ?? "Unknown Artist"}
                </div>
            </div>

            <div className="flex items-center gap-2" style={{ marginLeft: "auto", flexShrink: 0 }}>
                {item.song?.duration && (
                    <span className="mono text-muted" style={{ fontSize: 10 }}>
                        {formatDuration(item.song.duration)}
                    </span>
                )}
                <button
                    className="btn btn-ghost btn-icon"
                    style={{ width: 22, height: 22, opacity: 0.4 }}
                    onClick={() => item.id != null && onRemove(item.id)}
                    title="Remove from queue"
                >
                    <Trash2 size={11} />
                </button>
            </div>
        </div>
    );
}

export function QueuePanel() {
    const [items, setItems] = useState<QueueItem[]>([]);
    const [loading, setLoading] = useState(false);
    const [adding, setAdding] = useState(false);

    const sensors = useSensors(
        useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
        useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
    );

    const loadQueue = useCallback(async () => {
        setLoading(true);
        try {
            const q = await getQueue();
            setItems(q.map((e) => ({ ...e })));
        } catch (e) {
            console.error(e);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        loadQueue();
        const id = setInterval(loadQueue, 2000);
        const unsub = onDeckStateChanged((e) => {
            if (e.deck === "deck_a" || e.deck === "deck_b") {
                if (e.state === "ready" || e.state === "playing" || e.state === "idle") {
                    loadQueue();
                }
            }
        });
        return () => {
            clearInterval(id);
            unsub.then((f) => f());
        };
    }, [loadQueue]);

    const handleDragEnd = (event: DragEndEvent) => {
        const { active, over } = event;
        if (over && active.id !== over.id) {
            setItems((prev) => {
                const oldIdx = prev.findIndex((i) => (i.id ?? 0) === active.id);
                const newIdx = prev.findIndex((i) => (i.id ?? 0) === over.id);
                return arrayMove(prev, oldIdx, newIdx);
            });
        }
    };

    const handleRemove = async (queueId: number) => {
        try {
            await removeFromQueue(queueId);
            setItems((prev) => prev.filter((i) => i.id !== queueId));
        } catch (e) {
            console.error(e);
        }
    };

    const handleAutoAdd = async () => {
        setAdding(true);
        try {
            await enqueueNextClockwheelTrack();
            await loadQueue();
        } catch (e) {
            console.error(e);
        } finally {
            setAdding(false);
        }
    };

    const totalDuration = items.reduce((acc, i) => acc + (i.song?.duration ?? 0), 0);

    return (
        <div className="flex flex-col h-full">
            {/* Header */}
            <div
                className="flex items-center justify-between"
                style={{ padding: "6px 12px", borderBottom: "1px solid var(--border-default)", flexShrink: 0 }}
            >
                <div className="flex items-center gap-2">
                    <span className="section-label">Queue</span>
                    <span
                        className="mono"
                        style={{ fontSize: 10, color: "var(--amber)", background: "var(--amber-glow)", padding: "1px 6px", borderRadius: 10 }}
                    >
                        {items.length}
                    </span>
                </div>
                <div className="flex items-center gap-2">
                    {items.length > 0 && (
                        <div className="flex items-center gap-1 text-muted">
                            <Clock size={10} />
                            <span style={{ fontSize: 10 }}>{formatDuration(totalDuration)}</span>
                        </div>
                    )}
                    <button
                        className="btn btn-ghost btn-icon"
                        style={{ width: 24, height: 24 }}
                        onClick={handleAutoAdd}
                        disabled={adding}
                        title="Auto-add one song from clockwheel rotation"
                    >
                        <Plus size={12} />
                    </button>
                    <button className="btn btn-ghost" style={{ fontSize: 10, padding: "2px 8px" }} onClick={loadQueue}>
                        Refresh
                    </button>
                </div>
            </div>

            {/* Queue list */}
            <div className="overflow-auto flex-1" style={{ padding: "6px 8px" }}>
                {loading && items.length === 0 ? (
                    <div
                        className="flex items-center justify-center"
                        style={{ height: 80, color: "var(--text-muted)", fontSize: 12 }}
                    >
                        Loading queue…
                    </div>
                ) : items.length === 0 ? (
                    <div
                        className="flex flex-col items-center justify-center gap-3"
                        style={{ height: 80, color: "var(--text-muted)" }}
                    >
                        <Music2 size={20} />
                        <span style={{ fontSize: 11 }}>Queue is empty — drag tracks from library</span>
                    </div>
                ) : (
                    <DndContext
                        sensors={sensors}
                        collisionDetection={closestCenter}
                        onDragEnd={handleDragEnd}
                    >
                        <SortableContext
                            items={items.map((i, idx) => i.id ?? idx)}
                            strategy={verticalListSortingStrategy}
                        >
                            {items.map((item, idx) => (
                                <SortableQueueRow
                                    key={item.id ?? idx}
                                    item={item}
                                    index={idx}
                                    isNowPlaying={idx === 0}
                                    onRemove={handleRemove}
                                />
                            ))}
                        </SortableContext>
                    </DndContext>
                )}
            </div>
        </div>
    );
}
