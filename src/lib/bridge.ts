import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// ── Types ─────────────────────────────────────────────────────────────────────

export type DeckId = "deck_a" | "deck_b" | "sound_fx" | "aux_1" | "voice_fx";

export type FadeCurve =
  | "linear"
  | "exponential"
  | "s_curve"
  | "logarithmic"
  | "constant_power";

export type CrossfadeMode = "overlap" | "segue" | "instant";

export interface CrossfadeConfig {
  fade_out_enabled: boolean;
  fade_out_curve: FadeCurve;
  fade_out_time_ms: number;
  fade_in_enabled: boolean;
  fade_in_curve: FadeCurve;
  fade_in_time_ms: number;
  crossfade_mode: CrossfadeMode;
  auto_detect_enabled: boolean;
  auto_detect_db: number;
  auto_detect_min_ms: number;
  auto_detect_max_ms: number;
  fixed_crossfade_point_ms: number | null;
}

export interface CurvePoint {
  t: number;
  gain_out: number;
  gain_in: number;
}

export interface DeckStateEvent {
  deck: DeckId;
  state: string;
  position_ms: number;
  duration_ms: number;
}

export interface VuEvent {
  channel: DeckId;
  left_db: number;
  right_db: number;
}

export interface CrossfadeProgressEvent {
  progress: number;
  outgoing_deck: DeckId;
  incoming_deck: DeckId;
}

export interface CuePoint {
  id: number | null;
  song_id: number;
  name: string;
  position_ms: number;
}

export interface SamSong {
  id: number;           // SAM primary key (ID column)
  filename: string;
  songtype: string;     // 'S'=Song, 'J'=Jingle, etc.
  status: number;       // 0=disabled, 1=enabled
  weight: number;
  artist: string;
  title: string;
  album: string;
  genre: string;
  albumyear: string;
  duration: number;     // seconds
  bpm: number;
  xfade: string;        // crossfade preset name
  mood: string;
  mood_ai: string | null;
  rating: number;
  count_played: number;
  date_played: string | null;
  label: string;
  isrc: string;
  upc: string;          // also used as Spotify ID
  picture: string | null;
  overlay: string;      // 'yes' | 'no'
}

export interface QueueEntry {
  id: number;           // SAM queue primary key
  song_id: number;      // songID
  sort_id: number;      // sortID (float ordering)
  requests: number;
  request_id: number;
  plotw: number;        // 0=Song/PLO, 1=VoiceBreak/TW
  dedication: number;
}

export interface HistoryEntry {
  id: number;
  song_id: number;
  filename: string;
  date_played: string;  // MySQL datetime
  duration: number;
  artist: string;
  title: string;
  album: string;
  albumyear: string;
  listeners: number;
  label: string;
  isrc: string;
  upc: string;
  songtype: string;
  request_id: number;
  overlay: string;
  songrights: string;
}

export interface ChannelDspSettings {
  channel: string;
  eq_low_gain_db: number;
  eq_low_freq_hz: number;
  eq_mid_gain_db: number;
  eq_mid_freq_hz: number;
  eq_mid_q: number;
  eq_high_gain_db: number;
  eq_high_freq_hz: number;
  agc_enabled: boolean;
  agc_gate_db: number;
  agc_max_gain_db: number;
  agc_attack_ms: number;
  agc_release_ms: number;
  agc_pre_emphasis: string;
  comp_enabled: boolean;
  comp_settings_json: string | null;
}

// ── Deck control ─────────────────────────────────────────────────────────────

export const loadTrack = (deck: DeckId, filePath: string, songId?: number) =>
  invoke<void>("load_track", { deck, filePath, songId: songId ?? null });

export const playDeck = (deck: DeckId) => invoke<void>("play_deck", { deck });

export const pauseDeck = (deck: DeckId) => invoke<void>("pause_deck", { deck });

export const seekDeck = (deck: DeckId, positionMs: number) =>
  invoke<void>("seek_deck", { deck, positionMs });

export const setChannelGain = (deck: DeckId, gain: number) =>
  invoke<void>("set_channel_gain", { deck, gain });

export const getDeckState = (deck: DeckId) =>
  invoke<DeckStateEvent | null>("get_deck_state", { deck });

export const getVuReadings = () => invoke<VuEvent[]>("get_vu_readings");

// ── Crossfade ────────────────────────────────────────────────────────────────

export const getCrossfadeConfig = () =>
  invoke<CrossfadeConfig>("get_crossfade_config");

export const setCrossfadeConfig = (config: CrossfadeConfig) =>
  invoke<void>("set_crossfade_config", { config });

export const startCrossfade = (outgoing: DeckId, incoming: DeckId) =>
  invoke<void>("start_crossfade", { outgoing, incoming });

export const getFadeCurvePreview = (curve: FadeCurve, steps = 50) =>
  invoke<CurvePoint[]>("get_fade_curve_preview", { curve, steps });

// ── DSP ──────────────────────────────────────────────────────────────────────

export const getChannelDsp = (channel: DeckId) =>
  invoke<ChannelDspSettings | null>("get_channel_dsp", { channel });

export const setChannelEq = (
  channel: DeckId,
  lowGainDb: number,
  midGainDb: number,
  highGainDb: number
) =>
  invoke<void>("set_channel_eq", { channel, lowGainDb, midGainDb, highGainDb });

export const setChannelAgc = (
  channel: DeckId,
  enabled: boolean,
  gateDb?: number,
  maxGainDb?: number
) =>
  invoke<void>("set_channel_agc", {
    channel,
    enabled,
    gateDb: gateDb ?? null,
    maxGainDb: maxGainDb ?? null,
  });

// ── Cue points ───────────────────────────────────────────────────────────────

export const getCuePoints = (songId: number) =>
  invoke<CuePoint[]>("get_cue_points", { songId });

export const setCuePoint = (songId: number, name: string, positionMs: number) =>
  invoke<void>("set_cue_point", { songId, name, positionMs });

export const deleteCuePoint = (songId: number, name: string) =>
  invoke<void>("delete_cue_point", { songId, name });

export const jumpToCue = (deck: DeckId, songId: number, cueName: string) =>
  invoke<void>("jump_to_cue", { deck, songId, cueName });

// ── Queue / SAM ──────────────────────────────────────────────────────────────

export const getQueue = () => invoke<QueueEntry[]>("get_queue");

export const addToQueue = (songId: number) =>
  invoke<number>("add_to_queue", { songId });

export const removeFromQueue = (queueId: number) =>
  invoke<void>("remove_from_queue", { queueId });

/** Remove queue entry AND write a full history snapshot in one call. */
export const completeQueueItem = (queueId: number, songId: number) =>
  invoke<void>("complete_queue_item", { queueId, songId });

export const searchSongs = (query: string, limit = 50, offset = 0) =>
  invoke<SamSong[]>("search_songs", { query, limit, offset });

export const getHistory = (limit = 20) =>
  invoke<HistoryEntry[]>("get_history", { limit });

// ── SAM DB connection management ─────────────────────────────────────────────

export interface SamDbConnectArgs {
  host: string;
  port: number;
  username: string;
  password: string;
  database: string;
  auto_connect: boolean;
  path_prefix_from?: string;
  path_prefix_to?: string;
}

/** Saved connection config returned by the backend (no password). */
export interface SamDbConfig {
  host: string;
  port: number;
  username: string;
  database_name: string;
  auto_connect: boolean;
  /** Windows-style path prefix to strip (e.g. `C:\Music\`). Empty = no translation. */
  path_prefix_from: string;
  /** Local path to substitute (e.g. `/Volumes/Music/`). Empty = no translation. */
  path_prefix_to: string;
}

export interface SamDbStatus {
  connected: boolean;
  host: string | null;
  database: string | null;
  error: string | null;
}

export interface SamCategory {
  id: number;
  catname: string;
}

/** Test a connection without persisting anything. */
export const testSamDbConnection = (args: SamDbConnectArgs) =>
  invoke<SamDbStatus>("test_sam_db_connection", { args });

/** Connect, save config to SQLite, and store pool in AppState. */
export const connectSamDb = (args: SamDbConnectArgs) =>
  invoke<SamDbStatus>("connect_sam_db", { args });

/** Drop the pool. */
export const disconnectSamDb = () =>
  invoke<void>("disconnect_sam_db");

/** Return saved config (no password). */
export const getSamDbConfig = () =>
  invoke<SamDbConfig>("get_sam_db_config_cmd");

/** Persist config + password to SQLite without connecting. */
export const saveSamDbConfig = (config: SamDbConfig, password: string) =>
  invoke<void>("save_sam_db_config_cmd", { config, password });

/** Return live connection status. */
export const getSamDbStatus = () =>
  invoke<SamDbStatus>("get_sam_db_status");

/** Return SAM categories (empty array if catlist table absent). */
export const getSamCategories = () =>
  invoke<SamCategory[]>("get_sam_categories");

// ── Streaming ────────────────────────────────────────────────────────────────

export const startStream = (params: {
  host: string;
  port: number;
  mount: string;
  password: string;
  bitrateKbps: number;
  streamName?: string;
  genre?: string;
}) =>
  invoke<void>("start_stream", {
    host: params.host,
    port: params.port,
    mount: params.mount,
    password: params.password,
    bitrateKbps: params.bitrateKbps,
    streamName: params.streamName ?? null,
    genre: params.genre ?? null,
  });

export const stopStream = () => invoke<void>("stop_stream");

export const getStreamStatus = () => invoke<boolean>("get_stream_status");

// ── Event listeners ──────────────────────────────────────────────────────────

export const onDeckStateChanged = (
  cb: (event: DeckStateEvent) => void
): Promise<UnlistenFn> => listen<DeckStateEvent>("deck_state_changed", (e) => cb(e.payload));

export const onCrossfadeProgress = (
  cb: (event: CrossfadeProgressEvent) => void
): Promise<UnlistenFn> =>
  listen<CrossfadeProgressEvent>("crossfade_progress", (e) => cb(e.payload));

export const onVuMeter = (
  cb: (event: VuEvent) => void
): Promise<UnlistenFn> => listen<VuEvent>("vu_meter", (e) => cb(e.payload));

export const onStreamConnected = (
  cb: (mount: string) => void
): Promise<UnlistenFn> =>
  listen<{ mount: string }>("stream_connected", (e) => cb(e.payload.mount));

export const onStreamDisconnected = (
  cb: () => void
): Promise<UnlistenFn> => listen("stream_disconnected", () => cb());

// ── Phase 2 — Waveform ───────────────────────────────────────────────────────

export const getWaveformData = (filePath: string, resolution = 1000) =>
  invoke<number[]>("get_waveform_data", { filePath, resolution }).then(
    (arr) => new Float32Array(arr)
  );

// ── Phase 2 — Song details ───────────────────────────────────────────────────

/** Extended song detail — adds local-only metadata on top of SAM fields. */
export interface SongDetail extends SamSong {
  year: number | null;       // albumyear parsed as int (frontend convenience)
  comment: string | null;
  file_size: number | null;
  bitrate: number | null;
  fingerprint: string | null;
}

export const getSong = (songId: number) =>
  invoke<SongDetail>("get_song", { songId });

export const updateSong = (songId: number, fields: Partial<SamSong>) =>
  invoke<void>("update_song", { songId, fields });

export const getAlbumArt = (songId: number) =>
  invoke<string | null>("get_album_art", { songId });

// ── Phase 2 — Requests ───────────────────────────────────────────────────────

export interface RequestItem {
  requestId: number;
  songId: number;
  songTitle: string;
  artist: string;
  requesterName: string;
  requesterPlatform: string;
  requestedAt: number;
  status: "pending" | "accepted" | "rejected";
}

export const getRequests = (status: "pending" | "accepted" | "rejected" = "pending") =>
  invoke<RequestItem[]>("get_requests", { status });

export const acceptRequest = (requestId: number) =>
  invoke<void>("accept_request", { requestId });

export const rejectRequest = (requestId: number, reason?: string) =>
  invoke<void>("reject_request", { requestId, reason: reason ?? null });

// ── Phase 2 — New event listeners ────────────────────────────────────────────

export interface PlayheadUpdateEvent {
  deck: DeckId;
  positionMs: number;
}

export const onPlayheadUpdate = (
  cb: (event: PlayheadUpdateEvent) => void
): Promise<UnlistenFn> =>
  listen<PlayheadUpdateEvent>("playhead_update", (e) => cb(e.payload));

export const onQueueUpdated = (
  cb: () => void
): Promise<UnlistenFn> => listen("queue_updated", () => cb());

export const onRequestReceived = (
  cb: (request: RequestItem) => void
): Promise<UnlistenFn> =>
  listen<RequestItem>("request_received", (e) => cb(e.payload));

// ── Phase 4 — Encoder types ─────────────────────────────────────────────────

export type OutputType = "icecast" | "shoutcast" | "file";
export type EncoderCodec = "mp3" | "aac" | "ogg" | "wav" | "flac";
export type FileRotation = "none" | "hourly" | "daily" | "by_size";

export type EncoderStatusKind =
  | "disabled"
  | "connecting"
  | "streaming"
  | "retrying"
  | "failed"
  | "recording";

export interface EncoderStatusRetrying {
  retrying: { attempt: number; max: number };
}
export type EncoderStatus =
  | EncoderStatusKind
  | EncoderStatusRetrying;

export interface EncoderConfig {
  id: number;
  name: string;
  enabled: boolean;

  // Codec
  codec: EncoderCodec;
  bitrate_kbps: number | null;
  sample_rate: number;
  channels: number;
  quality: number | null;

  // Output
  output_type: OutputType;

  // Icecast / Shoutcast
  server_host: string | null;
  server_port: number | null;
  server_password: string | null;
  mount_point: string | null;
  stream_name: string | null;
  stream_genre: string | null;
  stream_url: string | null;
  stream_description: string | null;
  is_public: boolean;

  // File output
  file_output_path: string | null;
  file_rotation: FileRotation;
  file_max_size_mb: number;
  file_name_template: string;

  // Metadata
  send_metadata: boolean;
  icy_metadata_interval: number;

  // Reconnect
  reconnect_delay_secs: number;
  max_reconnect_attempts: number;
}

export interface EncoderRuntimeState {
  id: number;
  status: EncoderStatus;
  listeners: number | null;
  uptime_secs: number;
  bytes_sent: number;
  current_bitrate_kbps: number | null;
  error: string | null;
  recording_file: string | null;
}

export interface ListenerSnapshot {
  id: number | null;
  encoder_id: number;
  snapshot_at: number; // Unix timestamp
  current_listeners: number;
  peak_listeners: number;
  unique_listeners: number;
  stream_bitrate: number | null;
}

// ── Phase 4 — Encoder commands ──────────────────────────────────────────────

export const getEncoders = () =>
  invoke<EncoderConfig[]>("get_encoders");

export const saveEncoder = (encoder: EncoderConfig) =>
  invoke<number>("save_encoder", { encoder });

export const deleteEncoder = (id: number) =>
  invoke<void>("delete_encoder", { id });

export const startEncoder = (id: number) =>
  invoke<void>("start_encoder", { id });

export const stopEncoder = (id: number) =>
  invoke<void>("stop_encoder", { id });

export const startAllEncoders = () =>
  invoke<void>("start_all_encoders");

export const stopAllEncoders = () =>
  invoke<void>("stop_all_encoders");

export const testEncoderConnection = (id: number) =>
  invoke<boolean>("test_encoder_connection", { id });

export const getEncoderRuntime = () =>
  invoke<EncoderRuntimeState[]>("get_encoder_runtime");

// ── Phase 4 — Recording commands ────────────────────────────────────────────

export const startRecording = (encoderId: number) =>
  invoke<void>("start_recording", { encoderId });

export const stopRecording = (encoderId: number) =>
  invoke<void>("stop_recording", { encoderId });

// ── Phase 4 — Stats commands ────────────────────────────────────────────────

export type StatsPeriod = "1h" | "6h" | "24h" | "7d";

export const getListenerStats = (encoderId: number, period: StatsPeriod) =>
  invoke<ListenerSnapshot[]>("get_listener_stats", { encoderId, period });

export const getCurrentListeners = (encoderId: number) =>
  invoke<number>("get_current_listeners", { encoderId });

// ── Phase 4 — Metadata ──────────────────────────────────────────────────────

export const pushTrackMetadata = (artist: string, title: string) =>
  invoke<void>("push_track_metadata", { artist, title });

// ── Phase 4 — Events ────────────────────────────────────────────────────────

export interface EncoderStatusChangedEvent {
  id: number;
  status: EncoderStatus;
  listeners?: number;
  error?: string;
}

export interface ListenerCountUpdatedEvent {
  encoderId: number;
  count: number;
}

export interface RecordingRotationEvent {
  encoderId: number;
  closedFile: string;
  newFile: string;
}

export const onEncoderStatusChanged = (
  cb: (e: EncoderStatusChangedEvent) => void
): Promise<UnlistenFn> =>
  listen<EncoderStatusChangedEvent>("encoder_status_changed", (e) => cb(e.payload));

export const onListenerCountUpdated = (
  cb: (e: ListenerCountUpdatedEvent) => void
): Promise<UnlistenFn> =>
  listen<ListenerCountUpdatedEvent>("listener_count_updated", (e) => cb(e.payload));

export const onRecordingRotation = (
  cb: (e: RecordingRotationEvent) => void
): Promise<UnlistenFn> =>
  listen<RecordingRotationEvent>("recording_rotation", (e) => cb(e.payload));

// ═══════════════════════════════════════════════════════════════════════════
// Phase 3 — Automation & Scheduling
// ═══════════════════════════════════════════════════════════════════════════

// ── DJ Mode ──────────────────────────────────────────────────────────────────

export type DjMode = "autodj" | "assisted" | "manual";

export const getDjMode = (): Promise<DjMode> =>
  invoke<DjMode>("get_dj_mode");

export const setDjMode = (mode: DjMode): Promise<void> =>
  invoke<void>("set_dj_mode", { mode });

// ── Rotation Rules ────────────────────────────────────────────────────────────

export interface RotationRuleRow {
  id: number | null;
  name: string;
  rule_type: string;
  config_json: string;
  enabled: boolean;
  priority: number;
}

export const getRotationRules = (): Promise<RotationRuleRow[]> =>
  invoke<RotationRuleRow[]>("get_rotation_rules");

export const saveRotationRule = (rule: RotationRuleRow): Promise<number> =>
  invoke<number>("save_rotation_rule", { rule });

export const deleteRotationRule = (id: number): Promise<void> =>
  invoke<void>("delete_rotation_rule", { id });

export interface SongCandidate {
  song_id: number;
  title: string;
  artist: string;
  album: string | null;
  category: string | null;
  duration: number;
  file_path: string;
  score: number;
}

export const getNextAutoDjTrack = (): Promise<SongCandidate | null> =>
  invoke<SongCandidate | null>("get_next_autodj_track");

// ── Playlists ─────────────────────────────────────────────────────────────────

export interface Playlist {
  id: number | null;
  name: string;
  description: string | null;
  is_active: boolean;
  config_json: string;
}

export const getPlaylists = (): Promise<Playlist[]> =>
  invoke<Playlist[]>("get_playlists");

export const savePlaylist = (playlist: Playlist): Promise<number> =>
  invoke<number>("save_playlist", { playlist });

export const setActivePlaylist = (playlistId: number): Promise<void> =>
  invoke<void>("set_active_playlist", { playlistId });

// ── Show Scheduler ────────────────────────────────────────────────────────────

export type DayOfWeek =
  | "monday" | "tuesday" | "wednesday" | "thursday"
  | "friday" | "saturday" | "sunday";

export type ShowActionType =
  | { type: "play_playlist"; playlist_id: number }
  | { type: "play_song"; song_id: number }
  | { type: "start_stream"; encoder_id: string }
  | { type: "stop_stream"; encoder_id: string }
  | { type: "set_volume"; channel: string; volume: number }
  | { type: "switch_mode"; mode: DjMode }
  | { type: "play_jingle"; song_id: number };

export interface Show {
  id: number | null;
  name: string;
  days: DayOfWeek[];
  start_time: string; // "HH:MM"
  duration_minutes: number;
  actions: ShowActionType[];
  enabled: boolean;
}

export interface ScheduledEvent {
  show_id: number;
  show_name: string;
  fires_at: string; // ISO-8601
  actions: ShowActionType[];
}

export const getShows = (): Promise<Show[]> =>
  invoke<Show[]>("get_shows");

export const saveShow = (show: Show): Promise<number> =>
  invoke<number>("save_show", { show });

export const deleteShow = (id: number): Promise<void> =>
  invoke<void>("delete_show", { id });

export const getUpcomingEvents = (hours = 24): Promise<ScheduledEvent[]> =>
  invoke<ScheduledEvent[]>("get_upcoming_events", { hours });

// ── GAP Killer ────────────────────────────────────────────────────────────────

export interface GapKillerConfig {
  mode: "off" | "smart" | "aggressive";
  threshold_db: number;
  min_silence_ms: number;
}

export const getGapKillerConfig = (): Promise<GapKillerConfig> =>
  invoke<GapKillerConfig>("get_gap_killer_config");

export const setGapKillerConfig = (config: GapKillerConfig): Promise<void> =>
  invoke<void>("set_gap_killer_config", { config });

// ── Request Policy ────────────────────────────────────────────────────────────

export type RequestQueuePosition =
  | { type: "next" }
  | { type: "after"; n: number }
  | { type: "end" };

export interface RequestPolicy {
  max_requests_per_song_per_day: number;
  min_minutes_between_same_song: number;
  max_requests_per_artist_per_hour: number;
  min_minutes_between_same_artist: number;
  max_requests_per_album_per_day: number;
  max_requests_per_requester_per_day: number;
  max_requests_per_requester_per_hour: number;
  queue_position: RequestQueuePosition;
  blacklisted_song_ids: number[];
  blacklisted_categories: string[];
  active_hours: [number, number] | null;
  auto_accept: boolean;
}

export const getRequestPolicy = (): Promise<RequestPolicy> =>
  invoke<RequestPolicy>("get_request_policy");

export const setRequestPolicy = (policy: RequestPolicy): Promise<void> =>
  invoke<void>("set_request_policy", { policy });

// ── Request Log ───────────────────────────────────────────────────────────────

export type RequestStatusP3 = "pending" | "accepted" | "rejected" | "played";

export interface RequestLogEntry {
  id: number | null;
  song_id: number;
  song_title: string | null;
  artist: string | null;
  requester_name: string | null;
  requester_platform: string | null;
  requester_ip: string | null;
  requested_at: number;
  status: RequestStatusP3;
  rejection_reason: string | null;
  played_at: number | null;
}

export const getPendingRequests = (): Promise<RequestLogEntry[]> =>
  invoke<RequestLogEntry[]>("get_pending_requests");

export const acceptRequestP3 = (id: number): Promise<void> =>
  invoke<void>("accept_request_p3", { id });

export const rejectRequestP3 = (id: number, reason?: string): Promise<void> =>
  invoke<void>("reject_request_p3", { id, reason: reason ?? null });

export const getRequestHistoryLog = (limit = 100, offset = 0): Promise<RequestLogEntry[]> =>
  invoke<RequestLogEntry[]>("get_request_history", { limit, offset });

// ── Scheduler events ──────────────────────────────────────────────────────────

export interface ShowTriggeredEvent {
  show_id: number;
  show_name: string;
  action: ShowActionType;
}

export const onShowTriggered = (
  cb: (event: ShowTriggeredEvent) => void
): Promise<UnlistenFn> =>
  listen<ShowTriggeredEvent>("show_triggered", (e) => cb(e.payload));

export const onDjModeChanged = (
  cb: (mode: DjMode) => void
): Promise<UnlistenFn> =>
  listen<{ mode: DjMode }>("dj_mode_changed", (e) => cb(e.payload.mode));

