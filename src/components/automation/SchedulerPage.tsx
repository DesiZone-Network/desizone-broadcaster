import React, { useState } from "react";
import AutomationPanel from "./AutomationPanel";
import RotationRulesEditor from "./RotationRulesEditor";
import WeeklyCalendar from "./WeeklyCalendar";
import RequestPolicyEditor from "./RequestPolicyEditor";
import { Bot, Shuffle, CalendarClock, ShieldCheck } from "lucide-react";

type Tab = "automation" | "rotation" | "schedule" | "requests";

const TABS: { id: Tab; label: string; icon: React.ReactNode }[] = [
    { id: "automation", label: "Automation", icon: <Bot size={15} /> },
    { id: "rotation", label: "Rotation Rules", icon: <Shuffle size={15} /> },
    { id: "schedule", label: "Schedule", icon: <CalendarClock size={15} /> },
    { id: "requests", label: "Request Policy", icon: <ShieldCheck size={15} /> },
];

export default function SchedulerPage() {
    const [tab, setTab] = useState<Tab>("automation");

    return (
        <div className="scheduler-page">
            <div className="scheduler-tabs">
                {TABS.map((t) => (
                    <button
                        key={t.id}
                        className={`sched-tab${tab === t.id ? " active" : ""}`}
                        onClick={() => setTab(t.id)}
                    >
                        {t.icon}
                        {t.label}
                    </button>
                ))}
            </div>
            <div className="scheduler-content">
                {tab === "automation" && <AutomationPanel />}
                {tab === "rotation" && <RotationRulesEditor />}
                {tab === "schedule" && <WeeklyCalendar />}
                {tab === "requests" && <RequestPolicyEditor />}
            </div>
        </div>
    );
}
