import { useEffect, useState } from "react";
import {
    getRotationRules, saveRotationRule, deleteRotationRule,
    getPlaylists, savePlaylist, setActivePlaylist,
    RotationRuleRow, Playlist,
} from "../../lib/bridge";
import { Plus, Trash2, Check, List } from "lucide-react";

const RULE_TYPES = [
    { value: "artist_separation", label: "Artist Separation" },
    { value: "song_separation", label: "Song Separation" },
    { value: "album_separation", label: "Album Separation" },
    { value: "category_rotation", label: "Category Rotation" },
    { value: "max_plays_per_hour", label: "Max Plays / Hour" },
];

function RulesEditor() {
    const [rules, setRules] = useState<RotationRuleRow[]>([]);
    const [draft, setDraft] = useState<RotationRuleRow>({
        id: null,
        name: "",
        rule_type: "artist_separation",
        config_json: '{"minutes":45}',
        enabled: true,
        priority: 0,
    });

    const load = async () => {
        const r = await getRotationRules().catch(() => [] as RotationRuleRow[]);
        setRules(r);
    };

    useEffect(() => { load(); }, []);

    const save = async () => {
        if (!draft.name) return;
        await saveRotationRule(draft).catch(() => { });
        setDraft({ id: null, name: "", rule_type: "artist_separation", config_json: '{"minutes":45}', enabled: true, priority: 0 });
        load();
    };

    const del = async (id: number) => {
        await deleteRotationRule(id).catch(() => { });
        load();
    };

    return (
        <div className="rr-section">
            <h3 className="rr-subheader">Rotation Rules</h3>

            <div className="rr-rule-list">
                {rules.length === 0 ? (
                    <p className="rr-empty">No rotation rules defined — add one below.</p>
                ) : rules.map((r) => (
                    <div key={r.id} className="rr-rule-row">
                        <span className={`rr-badge${r.enabled ? "" : " disabled"}`}>
                            {RULE_TYPES.find((t) => t.value === r.rule_type)?.label ?? r.rule_type}
                        </span>
                        <span className="rr-rule-name">{r.name}</span>
                        <code className="rr-config">{r.config_json}</code>
                        <button className="rr-del-btn" onClick={() => r.id != null && del(r.id)}>
                            <Trash2 size={13} />
                        </button>
                    </div>
                ))}
            </div>

            <div className="rr-add-form">
                <input
                    placeholder="Rule name"
                    value={draft.name}
                    onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
                    className="rr-input"
                />
                <select
                    value={draft.rule_type}
                    onChange={(e) => setDraft((d) => ({ ...d, rule_type: e.target.value }))}
                    className="rr-select"
                >
                    {RULE_TYPES.map((t) => (
                        <option key={t.value} value={t.value}>{t.label}</option>
                    ))}
                </select>
                <input
                    placeholder='Config JSON e.g. {"minutes":45}'
                    value={draft.config_json}
                    onChange={(e) => setDraft((d) => ({ ...d, config_json: e.target.value }))}
                    className="rr-input"
                />
                <input
                    type="number"
                    placeholder="Priority"
                    value={draft.priority}
                    onChange={(e) => setDraft((d) => ({ ...d, priority: parseInt(e.target.value) || 0 }))}
                    className="rr-input rr-input-sm"
                />
                <button className="rr-add-btn" onClick={save}>
                    <Plus size={14} /> Add Rule
                </button>
            </div>
        </div>
    );
}

function PlaylistsEditor() {
    const [playlists, setPlaylists] = useState<Playlist[]>([]);
    const [newName, setNewName] = useState("");

    const load = async () => {
        const p = await getPlaylists().catch(() => [] as Playlist[]);
        setPlaylists(p);
    };

    useEffect(() => { load(); }, []);

    const create = async () => {
        if (!newName) return;
        await savePlaylist({ id: null, name: newName, description: null, is_active: false, config_json: "{}" }).catch(() => { });
        setNewName("");
        load();
    };

    const activate = async (id: number) => {
        await setActivePlaylist(id).catch(() => { });
        load();
    };

    return (
        <div className="rr-section">
            <h3 className="rr-subheader">Playlists</h3>
            <div className="rr-rule-list">
                {playlists.map((p) => (
                    <div key={p.id} className="rr-rule-row">
                        <List size={13} />
                        <span className="rr-rule-name">{p.name}</span>
                        {p.is_active && <span className="rr-active-badge">Active</span>}
                        <button
                            className="rr-add-btn"
                            onClick={() => p.id != null && activate(p.id)}
                            disabled={p.is_active}
                        >
                            <Check size={12} /> Use
                        </button>
                    </div>
                ))}
            </div>
            <div className="rr-add-form">
                <input
                    placeholder="New playlist name"
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                    className="rr-input"
                />
                <button className="rr-add-btn" onClick={create}>
                    <Plus size={14} /> Create
                </button>
            </div>
        </div>
    );
}

export default function RotationRulesEditor() {
    return (
        <div className="rotation-editor">
            <RulesEditor />
            <PlaylistsEditor />
        </div>
    );
}
