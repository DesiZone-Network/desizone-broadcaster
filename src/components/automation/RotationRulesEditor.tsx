import { useEffect, useMemo, useState } from "react";
import {
  ClockwheelConfig,
  ClockwheelSelectionMethod,
  ClockwheelSlot,
  ClockwheelSlotKind,
  SamCategory,
  getClockwheelConfig,
  getSamCategories,
  getSongDirectories,
  saveClockwheelConfig,
} from "../../lib/bridge";
import { Plus, Save, Trash2 } from "lucide-react";

const METHOD_OPTIONS: { value: ClockwheelSelectionMethod; label: string }[] = [
  { value: "weighted", label: "Weighted" },
  { value: "priority", label: "Priority" },
  { value: "random", label: "Random" },
  { value: "most_recently_played_song", label: "Most recently played song" },
  { value: "least_recently_played_song", label: "Least recently played song" },
  { value: "most_recently_played_artist", label: "Most recently played artist" },
  { value: "least_recently_played_artist", label: "Least recently played artist" },
  { value: "lemming", label: "Lemming rules (random logic)" },
  { value: "playlist_order", label: "Playlist order" },
];

const DAY_OPTIONS = [
  { value: 0, label: "Mon" },
  { value: 1, label: "Tue" },
  { value: 2, label: "Wed" },
  { value: 3, label: "Thu" },
  { value: 4, label: "Fri" },
  { value: 5, label: "Sat" },
  { value: 6, label: "Sun" },
];

const DEFAULT_CONFIG: ClockwheelConfig = {
  rules: {
    no_same_album_minutes: 15,
    no_same_artist_minutes: 8,
    no_same_title_minutes: 15,
    no_same_track_minutes: 180,
    keep_songs_in_queue: 1,
    use_ghost_queue: false,
    cache_queue_count: true,
    enforce_playlist_rotation_rules: true,
  },
  on_play_reduce_weight_by: 0,
  on_request_increase_weight_by: 0,
  verbose_logging: false,
  slots: [
    {
      id: "slot-1",
      kind: "category",
      target: "",
      selection_method: "weighted",
      enforce_rules: true,
      start_hour: null,
      end_hour: null,
      active_days: [],
    },
  ],
};

function cloneDefaultConfig(): ClockwheelConfig {
  return JSON.parse(JSON.stringify(DEFAULT_CONFIG)) as ClockwheelConfig;
}

function createSlot(kind: ClockwheelSlotKind, categories: SamCategory[]): ClockwheelSlot {
  const firstCategory = categories[0]?.catname ?? "";
  return {
    id: `slot-${Date.now()}-${Math.floor(Math.random() * 10000)}`,
    kind,
    target: kind === "category" ? firstCategory : "",
    selection_method: "weighted",
    enforce_rules: true,
    start_hour: null,
    end_hour: null,
    active_days: [],
  };
}

function hourValue(v: number | null): string {
  return typeof v === "number" ? String(v) : "";
}

function toClockwheelLine(slot: ClockwheelSlot): string {
  const methodToken = {
    weighted: "smWeighted",
    priority: "smPriority",
    random: "smRandom",
    most_recently_played_song: "smMostRecentSong",
    least_recently_played_song: "smLeastRecentSong",
    most_recently_played_artist: "smMostRecentArtist",
    least_recently_played_artist: "smLeastRecentArtist",
    lemming: "smLemmingLogic",
    playlist_order: "smPlaylistOrder",
  }[slot.selection_method];

  const enforce = slot.enforce_rules ? "EnforceRules" : "NoRules";
  const timePart =
    slot.start_hour != null && slot.end_hour != null
      ? ` // ${String(slot.start_hour).padStart(2, "0")}:00-${String(slot.end_hour).padStart(2, "0")}:00`
      : "";

  if (slot.kind === "request") {
    return `Req.QueueBottom;${timePart}`;
  }
  if (slot.kind === "directory") {
    return `Dir['${slot.target}'].QueueBottom(${methodToken}, ${enforce});${timePart}`;
  }
  return `Cat['${slot.target || "*"}'].QueueBottom(${methodToken}, ${enforce});${timePart}`;
}

export default function RotationRulesEditor() {
  const [config, setConfig] = useState<ClockwheelConfig>(cloneDefaultConfig());
  const [categories, setCategories] = useState<SamCategory[]>([]);
  const [directories, setDirectories] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    Promise.all([
      getClockwheelConfig().catch(() => cloneDefaultConfig()),
      getSamCategories().catch(() => [] as SamCategory[]),
      getSongDirectories(3000).catch(() => [] as string[]),
    ])
      .then(([cfg, cats, dirs]) => {
        if (cancelled) return;
        setConfig(cfg ?? cloneDefaultConfig());
        setCategories(cats);
        setDirectories(dirs);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const scriptPreview = useMemo(() => {
    const lines = config.slots.map(toClockwheelLine);
    return lines.join("\n");
  }, [config.slots]);

  const updateRule = (field: keyof ClockwheelConfig["rules"], value: number | boolean) => {
    setConfig((prev) => ({
      ...prev,
      rules: {
        ...prev.rules,
        [field]: value,
      },
    }));
  };

  const updateSlot = (index: number, patch: Partial<ClockwheelSlot>) => {
    setConfig((prev) => {
      const slots = [...prev.slots];
      slots[index] = { ...slots[index], ...patch };
      return { ...prev, slots };
    });
  };

  const toggleDay = (index: number, day: number) => {
    setConfig((prev) => {
      const slots = [...prev.slots];
      const existing = new Set(slots[index].active_days);
      if (existing.has(day)) {
        existing.delete(day);
      } else {
        existing.add(day);
      }
      slots[index] = { ...slots[index], active_days: Array.from(existing).sort((a, b) => a - b) };
      return { ...prev, slots };
    });
  };

  const addSlot = (kind: ClockwheelSlotKind) => {
    setConfig((prev) => ({
      ...prev,
      slots: [...prev.slots, createSlot(kind, categories)],
    }));
  };

  const removeSlot = (idx: number) => {
    setConfig((prev) => {
      const slots = prev.slots.filter((_, i) => i !== idx);
      return {
        ...prev,
        slots: slots.length > 0 ? slots : [createSlot("category", categories)],
      };
    });
  };

  const save = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await saveClockwheelConfig(config);
      setStatus("Clockwheel configuration saved.");
    } catch (e) {
      setStatus(`Save failed: ${String(e)}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="rotation-editor">
      <div className="rr-section">
        <h3 className="rr-subheader">Playlist Rules (SAM Style)</h3>

        {loading ? (
          <p className="rr-empty">Loading rotation configuration…</p>
        ) : (
          <>
            <div className="cw-rules-grid">
              <label className="cw-rule-field">
                <span>Do not play same album within (minutes)</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  value={config.rules.no_same_album_minutes}
                  onChange={(e) => updateRule("no_same_album_minutes", Math.max(0, parseInt(e.target.value || "0", 10)))}
                />
              </label>

              <label className="cw-rule-field">
                <span>Do not play same artist within (minutes)</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  value={config.rules.no_same_artist_minutes}
                  onChange={(e) => updateRule("no_same_artist_minutes", Math.max(0, parseInt(e.target.value || "0", 10)))}
                />
              </label>

              <label className="cw-rule-field">
                <span>Do not play same title within (minutes)</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  value={config.rules.no_same_title_minutes}
                  onChange={(e) => updateRule("no_same_title_minutes", Math.max(0, parseInt(e.target.value || "0", 10)))}
                />
              </label>

              <label className="cw-rule-field">
                <span>Do not play same track within (minutes)</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  value={config.rules.no_same_track_minutes}
                  onChange={(e) => updateRule("no_same_track_minutes", Math.max(0, parseInt(e.target.value || "0", 10)))}
                />
              </label>

              <label className="cw-rule-field">
                <span>Keep songs in queue</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  value={config.rules.keep_songs_in_queue}
                  onChange={(e) => updateRule("keep_songs_in_queue", Math.max(0, parseInt(e.target.value || "0", 10)))}
                />
              </label>

              <label className="cw-rule-field">
                <span>On play, reduce weight by</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  step={0.1}
                  value={config.on_play_reduce_weight_by}
                  onChange={(e) =>
                    setConfig((prev) => ({ ...prev, on_play_reduce_weight_by: Math.max(0, parseFloat(e.target.value || "0")) }))
                  }
                />
              </label>

              <label className="cw-rule-field">
                <span>On request, increase weight by</span>
                <input
                  className="rr-input rr-input-sm"
                  type="number"
                  min={0}
                  step={0.1}
                  value={config.on_request_increase_weight_by}
                  onChange={(e) =>
                    setConfig((prev) => ({ ...prev, on_request_increase_weight_by: Math.max(0, parseFloat(e.target.value || "0")) }))
                  }
                />
              </label>
            </div>

            <div className="cw-check-grid">
              <label><input type="checkbox" checked={config.rules.enforce_playlist_rotation_rules} onChange={(e) => updateRule("enforce_playlist_rotation_rules", e.target.checked)} /> Enforce playlist rotation rules</label>
              <label><input type="checkbox" checked={config.rules.use_ghost_queue} onChange={(e) => updateRule("use_ghost_queue", e.target.checked)} /> Use ghost queue</label>
              <label><input type="checkbox" checked={config.rules.cache_queue_count} onChange={(e) => updateRule("cache_queue_count", e.target.checked)} /> Cache queue count</label>
              <label><input type="checkbox" checked={config.verbose_logging} onChange={(e) => setConfig((prev) => ({ ...prev, verbose_logging: e.target.checked }))} /> Verbose logging</label>
            </div>
          </>
        )}
      </div>

      <div className="rr-section">
        <h3 className="rr-subheader">Rotation Clockwheel Format</h3>

        <div className="rr-add-form">
          <button className="rr-add-btn" onClick={() => addSlot("category")}><Plus size={14} /> Add Category</button>
          <button className="rr-add-btn" onClick={() => addSlot("directory")}><Plus size={14} /> Add Directory</button>
          <button className="rr-add-btn" onClick={() => addSlot("request")}><Plus size={14} /> Add Request</button>
        </div>

        <div className="cw-slot-list">
          {config.slots.map((slot, idx) => (
            <div key={slot.id} className="cw-slot-row">
              <div className="cw-slot-head">
                <strong>Slot {idx + 1}</strong>
                <button className="rr-del-btn" onClick={() => removeSlot(idx)}>
                  <Trash2 size={12} />
                </button>
              </div>

              <div className="cw-slot-grid">
                <label>
                  <span>Type</span>
                  <select
                    className="rr-select"
                    value={slot.kind}
                    onChange={(e) => updateSlot(idx, { kind: e.target.value as ClockwheelSlotKind, target: "" })}
                  >
                    <option value="category">Category</option>
                    <option value="directory">Directory</option>
                    <option value="request">Request</option>
                  </select>
                </label>

                <label>
                  <span>Source / Folder</span>
                  {slot.kind === "category" ? (
                    <select
                      className="rr-select"
                      value={slot.target}
                      onChange={(e) => updateSlot(idx, { target: e.target.value })}
                    >
                      <option value="">All categories</option>
                      {categories.map((c) => (
                        <option key={c.id} value={c.catname}>{c.catname}</option>
                      ))}
                    </select>
                  ) : slot.kind === "directory" ? (
                    <>
                      <input
                        className="rr-input"
                        value={slot.target}
                        onChange={(e) => updateSlot(idx, { target: e.target.value })}
                        list={`cw-dirs-${idx}`}
                        placeholder="/Music/Tracks"
                      />
                      <datalist id={`cw-dirs-${idx}`}>
                        {directories.map((d) => (
                          <option key={d} value={d} />
                        ))}
                      </datalist>
                    </>
                  ) : (
                    <input className="rr-input" value="Queue requests" disabled />
                  )}
                </label>

                <label>
                  <span>Selection method</span>
                  <select
                    className="rr-select"
                    value={slot.selection_method}
                    onChange={(e) => updateSlot(idx, { selection_method: e.target.value as ClockwheelSelectionMethod })}
                  >
                    {METHOD_OPTIONS.map((m) => (
                      <option key={m.value} value={m.value}>{m.label}</option>
                    ))}
                  </select>
                </label>

                <label className="cw-inline-check">
                  <input
                    type="checkbox"
                    checked={slot.enforce_rules}
                    onChange={(e) => updateSlot(idx, { enforce_rules: e.target.checked })}
                  />
                  <span>Enforce rules on this slot</span>
                </label>

                <label>
                  <span>Start hour (0-23)</span>
                  <input
                    className="rr-input rr-input-sm"
                    type="number"
                    min={0}
                    max={23}
                    value={hourValue(slot.start_hour)}
                    onChange={(e) => {
                      const v = e.target.value.trim();
                      updateSlot(idx, { start_hour: v === "" ? null : Math.max(0, Math.min(23, parseInt(v, 10) || 0)) });
                    }}
                  />
                </label>

                <label>
                  <span>End hour (0-23)</span>
                  <input
                    className="rr-input rr-input-sm"
                    type="number"
                    min={0}
                    max={23}
                    value={hourValue(slot.end_hour)}
                    onChange={(e) => {
                      const v = e.target.value.trim();
                      updateSlot(idx, { end_hour: v === "" ? null : Math.max(0, Math.min(23, parseInt(v, 10) || 0)) });
                    }}
                  />
                </label>
              </div>

              <div className="cw-days">
                {DAY_OPTIONS.map((d) => {
                  const active = slot.active_days.includes(d.value);
                  return (
                    <button
                      key={d.value}
                      className={`cw-day-btn${active ? " active" : ""}`}
                      onClick={() => toggleDay(idx, d.value)}
                    >
                      {d.label}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        <label className="cw-preview-wrap">
          <span>Clockwheel Preview</span>
          <textarea className="cw-preview" value={scriptPreview} readOnly rows={8} />
        </label>

        <div className="rr-add-form">
          <button className="rr-add-btn" onClick={save} disabled={saving}>
            <Save size={14} /> {saving ? "Saving…" : "Save Clockwheel"}
          </button>
          {status && <span className="cw-status">{status}</span>}
        </div>
      </div>
    </div>
  );
}
