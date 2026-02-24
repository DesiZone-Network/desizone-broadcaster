import { useEffect, useState } from "react";
import {
    getShows, saveShow, deleteShow,
    getUpcomingEvents,
    Show, ScheduledEvent, DayOfWeek,
} from "../../lib/bridge";
import { CalendarClock, Plus, Trash2, ChevronDown, ChevronUp } from "lucide-react";

const DAYS: DayOfWeek[] = [
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
];
const DAY_LABELS: Record<DayOfWeek, string> = {
    monday: "Mon", tuesday: "Tue", wednesday: "Wed",
    thursday: "Thu", friday: "Fri", saturday: "Sat", sunday: "Sun",
};

const emptyShow = (): Show => ({
    id: null,
    name: "",
    days: [],
    start_time: "08:00",
    duration_minutes: 60,
    actions: [],
    enabled: true,
});

function ShowCard({ show, onDelete }: { show: Show; onDelete: () => void }) {
    const [open, setOpen] = useState(false);
    return (
        <div className="sc-card">
            <div className="sc-card-header" onClick={() => setOpen((v) => !v)}>
                <CalendarClock size={14} />
                <span className="sc-show-name">{show.name}</span>
                <span className="sc-time">{show.start_time}</span>
                <div className="sc-days">
                    {show.days.length === 0
                        ? <span className="sc-badge">One-time</span>
                        : show.days.map((d) => <span key={d} className="sc-badge">{DAY_LABELS[d]}</span>)
                    }
                </div>
                {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </div>
            {open && (
                <div className="sc-card-body">
                    <p className="sc-detail">Duration: {show.duration_minutes} min</p>
                    <p className="sc-detail">Actions: {show.actions.length}</p>
                    <button className="rr-del-btn" onClick={onDelete}>
                        <Trash2 size={13} /> Delete
                    </button>
                </div>
            )}
        </div>
    );
}

function EventRow({ event }: { event: ScheduledEvent }) {
    const d = new Date(event.fires_at);
    return (
        <div className="sc-event-row">
            <span className="sc-event-time">
                {d.toLocaleDateString([], { weekday: "short" })} {d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
            </span>
            <span className="sc-event-name">{event.show_name}</span>
            <span className="sc-event-actions">{event.actions.length} action{event.actions.length !== 1 ? "s" : ""}</span>
        </div>
    );
}

export default function WeeklyCalendar() {
    const [shows, setShows] = useState<Show[]>([]);
    const [events, setEvents] = useState<ScheduledEvent[]>([]);
    const [draft, setDraft] = useState<Show>(emptyShow());
    const [creating, setCreating] = useState(false);

    const load = async () => {
        const [s, e] = await Promise.all([
            getShows().catch(() => [] as Show[]),
            getUpcomingEvents(48).catch(() => [] as ScheduledEvent[]),
        ]);
        setShows(s);
        setEvents(e);
    };

    useEffect(() => { load(); }, []);

    const toggleDay = (d: DayOfWeek) => {
        setDraft((prev) => ({
            ...prev,
            days: prev.days.includes(d) ? prev.days.filter((x) => x !== d) : [...prev.days, d],
        }));
    };

    const create = async () => {
        if (!draft.name) return;
        await saveShow(draft).catch(() => { });
        setDraft(emptyShow());
        setCreating(false);
        load();
    };

    const del = async (id: number) => {
        await deleteShow(id).catch(() => { });
        load();
    };

    return (
        <div className="weekly-calendar">
            <div className="sc-col sc-shows">
                <div className="ap-section-header">
                    <CalendarClock size={16} />
                    <span>Scheduled Shows</span>
                    <button className="rr-add-btn sc-new-btn" onClick={() => setCreating((v) => !v)}>
                        <Plus size={13} /> New
                    </button>
                </div>

                {creating && (
                    <div className="sc-create-form">
                        <input
                            className="rr-input"
                            placeholder="Show name"
                            value={draft.name}
                            onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
                        />
                        <div className="sc-day-picker">
                            {DAYS.map((day) => (
                                <button
                                    key={day}
                                    className={`sc-day-btn${draft.days.includes(day) ? " active" : ""}`}
                                    onClick={() => toggleDay(day)}
                                >
                                    {DAY_LABELS[day]}
                                </button>
                            ))}
                        </div>
                        <div className="rr-add-form">
                            <label className="ap-field">
                                <span>Time</span>
                                <input
                                    type="time"
                                    value={draft.start_time}
                                    onChange={(e) => setDraft((d) => ({ ...d, start_time: e.target.value }))}
                                    className="rr-input"
                                />
                            </label>
                            <label className="ap-field">
                                <span>Duration (min)</span>
                                <input
                                    type="number"
                                    min={0}
                                    value={draft.duration_minutes}
                                    onChange={(e) => setDraft((d) => ({ ...d, duration_minutes: parseInt(e.target.value) || 0 }))}
                                    className="rr-input rr-input-sm"
                                />
                            </label>
                        </div>
                        <button className="rr-add-btn" onClick={create}>Save Show</button>
                    </div>
                )}

                <div className="sc-show-list">
                    {shows.length === 0
                        ? <p className="rr-empty">No shows scheduled.</p>
                        : shows.map((s) => (
                            <ShowCard key={s.id} show={s} onDelete={() => s.id != null && del(s.id)} />
                        ))
                    }
                </div>
            </div>

            <div className="sc-col sc-upcoming">
                <div className="ap-section-header">
                    <CalendarClock size={16} />
                    <span>Upcoming (48h)</span>
                </div>
                {events.length === 0
                    ? <p className="rr-empty">No events in the next 48 hours.</p>
                    : events.map((ev, i) => <EventRow key={i} event={ev} />)
                }
            </div>
        </div>
    );
}
