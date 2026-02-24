import { useEffect, useState } from "react";
import {
    getRequestPolicy, setRequestPolicy,
    RequestPolicy,
} from "../../lib/bridge";
import { ShieldCheck } from "lucide-react";

const defaultPolicy = (): RequestPolicy => ({
    max_requests_per_song_per_day: 3,
    min_minutes_between_same_song: 60,
    max_requests_per_artist_per_hour: 2,
    min_minutes_between_same_artist: 30,
    max_requests_per_album_per_day: 5,
    max_requests_per_requester_per_day: 5,
    max_requests_per_requester_per_hour: 2,
    queue_position: { type: "end" },
    blacklisted_song_ids: [],
    blacklisted_categories: [],
    active_hours: null,
    auto_accept: false,
});

function NumberPolicyField({
    label, value, min = 0, max = 999,
    onChange,
}: {
    label: string; value: number; min?: number; max?: number;
    onChange: (v: number) => void;
}) {
    return (
        <label className="rp-field">
            <span>{label}</span>
            <input
                type="number"
                min={min}
                max={max}
                value={value}
                onChange={(e) => onChange(parseInt(e.target.value) || 0)}
                className="rr-input rr-input-sm"
            />
        </label>
    );
}

export default function RequestPolicyEditor() {
    const [policy, setPolicy] = useState<RequestPolicy>(defaultPolicy());
    const [saving, setSaving] = useState(false);
    const [blackCatInput, setBlackCatInput] = useState("");

    useEffect(() => {
        getRequestPolicy().then(setPolicy).catch(() => { });
    }, []);

    const save = async () => {
        setSaving(true);
        await setRequestPolicy(policy).catch(() => { });
        setSaving(false);
    };

    const addBlackCategory = () => {
        if (!blackCatInput) return;
        setPolicy((p) => ({
            ...p,
            blacklisted_categories: [...p.blacklisted_categories, blackCatInput],
        }));
        setBlackCatInput("");
    };

    const removeBlackCategory = (cat: string) => {
        setPolicy((p) => ({
            ...p,
            blacklisted_categories: p.blacklisted_categories.filter((c) => c !== cat),
        }));
    };

    return (
        <div className="request-policy-editor">
            <div className="ap-section-header">
                <ShieldCheck size={16} />
                <span>Request Policy</span>
            </div>

            <div className="rp-grid">
                {/* Song limits */}
                <div className="rp-group">
                    <h4 className="rp-group-title">Song Limits</h4>
                    <NumberPolicyField
                        label="Max req/song/day"
                        value={policy.max_requests_per_song_per_day}
                        onChange={(v) => setPolicy((p) => ({ ...p, max_requests_per_song_per_day: v }))}
                    />
                    <NumberPolicyField
                        label="Min gap between same song (min)"
                        value={policy.min_minutes_between_same_song}
                        onChange={(v) => setPolicy((p) => ({ ...p, min_minutes_between_same_song: v }))}
                    />
                </div>

                {/* Artist limits */}
                <div className="rp-group">
                    <h4 className="rp-group-title">Artist Limits</h4>
                    <NumberPolicyField
                        label="Max req/artist/hour"
                        value={policy.max_requests_per_artist_per_hour}
                        onChange={(v) => setPolicy((p) => ({ ...p, max_requests_per_artist_per_hour: v }))}
                    />
                    <NumberPolicyField
                        label="Min gap between same artist (min)"
                        value={policy.min_minutes_between_same_artist}
                        onChange={(v) => setPolicy((p) => ({ ...p, min_minutes_between_same_artist: v }))}
                    />
                    <NumberPolicyField
                        label="Max req/album/day"
                        value={policy.max_requests_per_album_per_day}
                        onChange={(v) => setPolicy((p) => ({ ...p, max_requests_per_album_per_day: v }))}
                    />
                </div>

                {/* Requester limits */}
                <div className="rp-group">
                    <h4 className="rp-group-title">Requester Limits</h4>
                    <NumberPolicyField
                        label="Max req/requester/day"
                        value={policy.max_requests_per_requester_per_day}
                        onChange={(v) => setPolicy((p) => ({ ...p, max_requests_per_requester_per_day: v }))}
                    />
                    <NumberPolicyField
                        label="Max req/requester/hour"
                        value={policy.max_requests_per_requester_per_hour}
                        onChange={(v) => setPolicy((p) => ({ ...p, max_requests_per_requester_per_hour: v }))}
                    />
                </div>

                {/* Queue position */}
                <div className="rp-group">
                    <h4 className="rp-group-title">Queue Position</h4>
                    <label className="rp-field">
                        <span>Where to insert request</span>
                        <select
                            value={(policy.queue_position as any).type}
                            onChange={(e) => {
                                const type = e.target.value as "next" | "after" | "end";
                                setPolicy((p) => ({
                                    ...p,
                                    queue_position: type === "after"
                                        ? { type: "after", n: 2 }
                                        : { type },
                                }));
                            }}
                            className="rr-select"
                        >
                            <option value="next">Next (play immediately after current)</option>
                            <option value="after">After N songs</option>
                            <option value="end">End of queue</option>
                        </select>
                    </label>
                    {(policy.queue_position as any).type === "after" && (
                        <NumberPolicyField
                            label="After how many songs"
                            value={(policy.queue_position as any).n ?? 2}
                            min={1}
                            max={50}
                            onChange={(v) => setPolicy((p) => ({ ...p, queue_position: { type: "after", n: v } }))}
                        />
                    )}
                </div>

                {/* Blacklisted categories */}
                <div className="rp-group rp-group-wide">
                    <h4 className="rp-group-title">Blacklisted Categories</h4>
                    <div className="rp-tags">
                        {policy.blacklisted_categories.map((cat) => (
                            <span key={cat} className="rp-tag">
                                {cat}
                                <button onClick={() => removeBlackCategory(cat)}>×</button>
                            </span>
                        ))}
                    </div>
                    <div className="rr-add-form">
                        <input
                            placeholder="Category name"
                            value={blackCatInput}
                            onChange={(e) => setBlackCatInput(e.target.value)}
                            className="rr-input"
                            onKeyDown={(e) => e.key === "Enter" && addBlackCategory()}
                        />
                        <button className="rr-add-btn" onClick={addBlackCategory}>Add</button>
                    </div>
                </div>

                {/* Active hours */}
                <div className="rp-group">
                    <h4 className="rp-group-title">Request Hours</h4>
                    <label className="rp-field">
                        <span>Restrict to hours</span>
                        <input
                            type="checkbox"
                            checked={policy.active_hours !== null}
                            onChange={(e) =>
                                setPolicy((p) => ({
                                    ...p,
                                    active_hours: e.target.checked ? [8, 22] : null,
                                }))
                            }
                        />
                    </label>
                    {policy.active_hours && (
                        <div className="rr-add-form">
                            <NumberPolicyField
                                label="From (hour)"
                                value={policy.active_hours[0]}
                                min={0} max={23}
                                onChange={(v) =>
                                    setPolicy((p) => ({
                                        ...p,
                                        active_hours: [v, (p.active_hours ?? [8, 22])[1]],
                                    }))
                                }
                            />
                            <NumberPolicyField
                                label="To (hour)"
                                value={policy.active_hours[1]}
                                min={0} max={24}
                                onChange={(v) =>
                                    setPolicy((p) => ({
                                        ...p,
                                        active_hours: [(p.active_hours ?? [8, 22])[0], v],
                                    }))
                                }
                            />
                        </div>
                    )}
                </div>

                {/* Auto-accept */}
                <div className="rp-group">
                    <h4 className="rp-group-title">Acceptance</h4>
                    <label className="rp-field rp-toggle">
                        <span>Auto-accept requests that pass all rules</span>
                        <input
                            type="checkbox"
                            checked={policy.auto_accept}
                            onChange={(e) => setPolicy((p) => ({ ...p, auto_accept: e.target.checked }))}
                        />
                    </label>
                </div>
            </div>

            <button className="ap-save-btn" onClick={save} disabled={saving}>
                {saving ? "Saving…" : "Save Policy"}
            </button>
        </div>
    );
}
