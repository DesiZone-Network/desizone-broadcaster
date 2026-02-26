import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// ── Types ─────────────────────────────────────────────────────────────────────

export type DeckId = "deck_a" | "deck_b" | "sound_fx" | "aux_1" | "aux_2" | "voice_fx";

export type FadeCurve =
  | "linear"
  | "exponential"
  | "s_curve"
  | "logarithmic"
  | "constant_power";

export type CrossfadeMode = "overlap" | "segue" | "instant";
export type CrossfadeTriggerMode = "auto_detect_db" | "fixed_point_ms" | "manual";

export interface CrossfadeConfig {
  fade_out_enabled: boolean;
  fade_out_curve: FadeCurve;
  fade_out_time_ms: number;
  fade_out_level_pct: number;
  fade_in_enabled: boolean;
  fade_in_curve: FadeCurve;
  fade_in_time_ms: number;
  fade_in_level_pct: number;
  crossfade_mode: CrossfadeMode;
  trigger_mode: CrossfadeTriggerMode;
  fixed_crossfade_ms: number;
  auto_detect_enabled: boolean;
  auto_detect_db: number;
  min_fade_time_ms: number;
  max_fade_time_ms: number;
  skip_short_tracks_secs: number | null;
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
  song_id?: number | null;
  file_path?: string | null;
  playback_rate?: number;
  pitch_pct?: number;
  tempo_pct?: number;
  decoder_buffer_ms: number;
  rms_db_pre_fader: number;
  cue_preview_enabled?: boolean;
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
  cue_kind?: "hotcue" | "memory" | "transition";
  slot?: number | null;
  label?: string;
  color_hex?: string;
  updated_at?: number | null;
}

export type CueQuantize = "off" | "beat_1" | "beat_half" | "beat_quarter";

export interface HotCue {
  song_id: number;
  slot: number;
  position_ms: number;
  label: string;
  color_hex: string;
  quantized: boolean;
}

export interface BeatGridAnalysis {
  song_id: number;
  file_path: string;
  mtime_ms: number;
  bpm: number;
  first_beat_ms: number;
  confidence: number;
  beat_times_ms: number[];
  updated_at?: number | null;
}

export interface StemAnalysis {
  song_id: number;
  source_file_path: string;
  source_mtime_ms: number;
  vocals_file_path: string;
  instrumental_file_path: string;
  model_name: string;
  updated_at?: number | null;
}

export type StemPlaybackSource = "original" | "vocals" | "instrumental";

export interface DeckStemSourceResult {
  source: StemPlaybackSource;
  file_path: string;
}

export interface StemsRuntimeStatus {
  ready: boolean;
  runtime_dir: string;
  python_path: string | null;
  ffmpeg_available: boolean;
  message: string;
}

export interface MonitorRoutingConfig {
  master_device_id: string | null;
  cue_device_id: string | null;
  cue_mix_mode: string;
  cue_level: number;
  master_level: number;
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
  song?: SamSong;       // hydrated from songlist when available
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
  pipeline_settings_json?: string | null;
}

export interface BandConfig {
  threshold_db: number;
  ratio: number;
  knee_db: number;
  attack_ms: number;
  release_ms: number;
  makeup_db: number;
}

export interface PipelineSettings {
  eq: {
    low_gain_db: number;
    low_freq_hz: number;
    mid_gain_db: number;
    mid_freq_hz: number;
    mid_q: number;
    high_gain_db: number;
    high_freq_hz: number;
  };
  agc: {
    enabled: boolean;
    gate_db: number;
    max_gain_db: number;
    target_db: number;
    attack_ms: number;
    release_ms: number;
    pre_emphasis: "none" | "us50" | "us75";
  };
  multiband: {
    enabled: boolean;
    bands: BandConfig[];
  };
  dual_band: {
    enabled: boolean;
    crossover_hz: number;
    lf_band: BandConfig;
    hf_band: BandConfig;
  };
  clipper: {
    enabled: boolean;
    ceiling_db: number;
  };
  stem_filter: {
    mode: "off" | "vocal" | "instrumental";
    amount: number;
  };
}
export type StemFilterMode = "off" | "vocal" | "instrumental";

// ── Deck control ─────────────────────────────────────────────────────────────

export const loadTrack = (deck: DeckId, filePath: string, songId?: number) =>
  invoke<void>("load_track", { deck, filePath, songId: songId ?? null });

export const playDeck = (deck: DeckId) => invoke<void>("play_deck", { deck });

export const pauseDeck = (deck: DeckId) => invoke<void>("pause_deck", { deck });

export const stopDeck = (deck: DeckId) => invoke<void>("stop_deck", { deck });

export const nextDeck = (deck: DeckId) => invoke<void>("next_deck", { deck });

export const seekDeck = (deck: DeckId, positionMs: number) =>
  invoke<void>("seek_deck", { deck, positionMs });

export const setChannelGain = (deck: DeckId, gain: number) =>
  invoke<void>("set_channel_gain", { deck, gain });

export const setDeckPitch = (deck: DeckId, pitchPct: number) =>
  invoke<void>("set_deck_pitch", { deck, pitchPct });

export const setDeckTempo = (deck: DeckId, tempoPct: number) =>
  invoke<void>("set_deck_tempo", { deck, tempoPct });

export const setDeckLoop = (deck: DeckId, startMs: number, endMs: number) =>
  invoke<void>("set_deck_loop", { deck, startMs, endMs });

export const clearDeckLoop = (deck: DeckId) =>
  invoke<void>("clear_deck_loop", { deck });

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

export const setManualCrossfade = (position: number) =>
  invoke<void>("set_manual_crossfade", { position });

export const triggerManualFade = (direction: "a_to_b" | "b_to_a", durationMs: number) =>
  invoke<void>("trigger_manual_fade", { direction, durationMs });

export const getFadeCurvePreview = (curve: FadeCurve, steps = 50) =>
  invoke<CurvePoint[]>("get_fade_curve_preview", { curve, steps });

// ── DSP ──────────────────────────────────────────────────────────────────────

export const getChannelDsp = (channel: DeckId | "master") =>
  invoke<ChannelDspSettings | null>("get_channel_dsp", { channel });

export const setChannelEq = (
  channel: DeckId,
  lowGainDb: number,
  midGainDb: number,
  highGainDb: number
) =>
  invoke<void>("set_channel_eq", { channel, lowGainDb, midGainDb, highGainDb });

export const setChannelAgc = (
  channel: DeckId | "master",
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

export const setPipelineSettings = (
  channel: DeckId | "master",
  settings: PipelineSettings
) =>
  invoke<void>("set_pipeline_settings", { channel, settings });

export const setChannelStemFilter = (
  channel: DeckId | "master",
  mode: StemFilterMode,
  amount?: number
) =>
  invoke<void>("set_channel_stem_filter", {
    channel,
    mode,
    amount: amount ?? null,
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

export const getHotCues = (songId: number) =>
  invoke<HotCue[]>("get_hot_cues", { songId });

export const setHotCue = (
  songId: number,
  slot: number,
  positionMs: number,
  label?: string,
  colorHex?: string,
  quantizeMode?: CueQuantize
) =>
  invoke<HotCue>("set_hot_cue", {
    songId,
    slot,
    positionMs,
    label: label ?? null,
    colorHex: colorHex ?? null,
    quantizeMode: quantizeMode ?? null,
  });

export const clearHotCue = (songId: number, slot: number) =>
  invoke<void>("clear_hot_cue", { songId, slot });

export const triggerHotCue = (
  deck: DeckId,
  songId: number,
  slot: number,
  quantizeMode?: CueQuantize
) =>
  invoke<HotCue>("trigger_hot_cue", {
    deck,
    songId,
    slot,
    quantizeMode: quantizeMode ?? null,
  });

export const renameHotCue = (songId: number, slot: number, label: string) =>
  invoke<void>("rename_hot_cue", { songId, slot, label });

export const recolorHotCue = (songId: number, slot: number, colorHex: string) =>
  invoke<void>("recolor_hot_cue", { songId, slot, colorHex });

export const analyzeBeatgrid = (
  songId: number,
  filePath: string,
  forceReanalyze = false
) =>
  invoke<BeatGridAnalysis>("analyze_beatgrid", { songId, filePath, forceReanalyze });

export const getBeatgrid = (songId: number, filePath: string) =>
  invoke<BeatGridAnalysis | null>("get_beatgrid", { songId, filePath });

export const analyzeStems = (
  songId: number,
  filePath: string,
  forceReanalyze = false
) =>
  invoke<StemAnalysis>("analyze_stems", { songId, filePath, forceReanalyze });

export const getStemAnalysis = (songId: number, filePath: string) =>
  invoke<StemAnalysis | null>("get_stem_analysis", { songId, filePath });

export const getLatestStemAnalysis = (songId: number) =>
  invoke<StemAnalysis | null>("get_latest_stem_analysis", { songId });

export const getStemsRuntimeStatus = () =>
  invoke<StemsRuntimeStatus>("get_stems_runtime_status");

export const installStemsRuntime = () =>
  invoke<StemsRuntimeStatus>("install_stems_runtime");

export const setDeckStemSource = (
  deck: DeckId,
  source: StemPlaybackSource,
  songId?: number,
  originalFilePath?: string
) =>
  invoke<DeckStemSourceResult>("set_deck_stem_source", {
    deck,
    source,
    songId: songId ?? null,
    originalFilePath: originalFilePath ?? null,
  });

export const getMonitorRoutingConfig = () =>
  invoke<MonitorRoutingConfig>("get_monitor_routing_config");

export const setMonitorRoutingConfig = (config: MonitorRoutingConfig) =>
  invoke<void>("set_monitor_routing_config", { config });

export const setDeckCuePreviewEnabled = (deck: DeckId, enabled: boolean) =>
  invoke<void>("set_deck_cue_preview_enabled", { deck, enabled });

// ── Queue / SAM ──────────────────────────────────────────────────────────────

export const getQueue = () => invoke<QueueEntry[]>("get_queue");

export const addToQueue = (songId: number) =>
  invoke<number>("add_to_queue", { songId });

export const removeFromQueue = (queueId: number) =>
  invoke<void>("remove_from_queue", { queueId });

export const reorderQueue = (queueIds: number[]) =>
  invoke<void>("reorder_queue", { queueIds });

/** Remove queue entry AND write a full history snapshot in one call. */
export const completeQueueItem = (queueId: number, songId: number) =>
  invoke<void>("complete_queue_item", { queueId, songId });

export interface SearchSongsOptions {
  searchArtist?: boolean;
  searchTitle?: boolean;
  searchAlbum?: boolean;
  searchFilename?: boolean;
  songType?: string | null;
}

export const searchSongs = (
  query: string,
  limit = 500,
  offset = 0,
  options?: SearchSongsOptions
) =>
  invoke<SamSong[]>("search_songs", {
    query,
    limit,
    offset,
    searchArtist: options?.searchArtist ?? true,
    searchTitle: options?.searchTitle ?? true,
    searchAlbum: options?.searchAlbum ?? false,
    searchFilename: options?.searchFilename ?? false,
    songType: options?.songType ?? null,
  });

export const getSongsByWeightRange = (
  minWeight: number,
  maxWeight: number,
  limit = 500,
  offset = 0
) =>
  invoke<SamSong[]>("get_songs_by_weight_range", { minWeight, maxWeight, limit, offset });

export const getSongTypes = () => invoke<string[]>("get_song_types");

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
  parent_id: number;
  levelindex: number;
  itemindex: number;
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

/** Return SAM categories (supports both `category` and legacy `catlist`). */
export const getSamCategories = () =>
  invoke<SamCategory[]>("get_sam_categories");

/** Return songs belonging to a SAM category via the categorylist join table. */
export const getSongsInCategory = (categoryId: number, limit = 500, offset = 0) =>
  invoke<SamSong[]>("get_songs_in_category", { categoryId, limit, offset });

/** Create a folder/category in SAM (`category` or `catlist`). */
export const createSamCategory = (name: string, parentId?: number | null) =>
  invoke<SamCategory>("create_sam_category", { name, parentId: parentId ?? null });


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
  invoke<SamSong | null>("get_song", { songId });

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

export const setDjMode = async (mode: DjMode): Promise<void> => {
  await invoke<void>("set_dj_mode", { mode });
  window.dispatchEvent(new CustomEvent<DjMode>("dj_mode_changed", { detail: mode }));
};

export type AutoTransitionMode =
  | "full_intro_outro"
  | "fade_at_outro_start"
  | "fixed_full_track"
  | "fixed_skip_silence"
  | "fixed_start_center_skip_silence";

export type AutodjTransitionEngine = "sam_classic" | "mixxx_planner";

export interface MixxxPlannerConfig {
  enabled: boolean;
  mode: AutoTransitionMode;
  transition_time_sec: number;
  min_track_duration_ms: number;
}

export interface AutoTransitionConfig {
  engine: AutodjTransitionEngine;
  mixxx_planner_config: MixxxPlannerConfig;
}

export interface TransitionDecisionDebug {
  engine: string;
  from_deck: string | null;
  to_deck: string | null;
  trigger_mode: string | null;
  reason: string;
  outgoing_rms_db: number | null;
  threshold_db: number | null;
  outgoing_remaining_ms: number | null;
  fixed_point_ms: number | null;
  hold_ms: number | null;
  skip_cause: string | null;
}

export const getAutoDjTransitionConfig = (): Promise<AutoTransitionConfig> =>
  invoke<AutoTransitionConfig>("get_autodj_transition_config");

export const setAutoDjTransitionConfig = (config: AutoTransitionConfig): Promise<void> =>
  invoke<void>("set_autodj_transition_config", { config });

export const recalculateAutoDjPlanNow = (): Promise<void> =>
  invoke<void>("recalculate_autodj_plan_now");

export const getLastTransitionDecision = (): Promise<TransitionDecisionDebug> =>
  invoke<TransitionDecisionDebug>("get_last_transition_decision");

export const onDjModeChanged = (cb: (mode: DjMode) => void) => {
  const handler = (e: Event) => {
    const ev = e as CustomEvent<DjMode>;
    cb(ev.detail);
  };
  window.addEventListener("dj_mode_changed", handler);
  return () => window.removeEventListener("dj_mode_changed", handler);
};

// ── Rotation Rules ────────────────────────────────────────────────────────────

export interface RotationRuleRow {
  id: number | null;
  name: string;
  rule_type: string;
  config_json: string;
  enabled: boolean;
  priority: number;
}

export type ClockwheelSlotKind = "category" | "directory" | "request";

export type ClockwheelSelectionMethod =
  | "weighted"
  | "priority"
  | "random"
  | "most_recently_played_song"
  | "least_recently_played_song"
  | "most_recently_played_artist"
  | "least_recently_played_artist"
  | "lemming"
  | "playlist_order";

export interface ClockwheelSlot {
  id: string;
  kind: ClockwheelSlotKind;
  target: string;
  selection_method: ClockwheelSelectionMethod;
  enforce_rules: boolean;
  start_hour: number | null;
  end_hour: number | null;
  active_days: number[];
}

export interface ClockwheelRules {
  no_same_album_minutes: number;
  no_same_artist_minutes: number;
  no_same_title_minutes: number;
  no_same_track_minutes: number;
  keep_songs_in_queue: number;
  use_ghost_queue: boolean;
  cache_queue_count: boolean;
  enforce_playlist_rotation_rules: boolean;
}

export interface ClockwheelConfig {
  rules: ClockwheelRules;
  on_play_reduce_weight_by: number;
  on_request_increase_weight_by: number;
  verbose_logging: boolean;
  slots: ClockwheelSlot[];
}

export const getRotationRules = (): Promise<RotationRuleRow[]> =>
  invoke<RotationRuleRow[]>("get_rotation_rules");

export const saveRotationRule = (rule: RotationRuleRow): Promise<number> =>
  invoke<number>("save_rotation_rule", { rule });

export const deleteRotationRule = (id: number): Promise<void> =>
  invoke<void>("delete_rotation_rule", { id });

export const getClockwheelConfig = (): Promise<ClockwheelConfig> =>
  invoke<ClockwheelConfig>("get_clockwheel_config");

export const saveClockwheelConfig = (config: ClockwheelConfig): Promise<void> =>
  invoke<void>("save_clockwheel_config", { config });

export const getSongDirectories = (limit = 3000): Promise<string[]> =>
  invoke<string[]>("get_song_directories", { limit });

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

export interface EnqueuedClockwheelTrack {
  queue_id: number;
  song: SongCandidate;
}

export const enqueueNextClockwheelTrack = (
  slotId?: string
): Promise<EnqueuedClockwheelTrack | null> =>
  invoke<EnqueuedClockwheelTrack | null>("enqueue_next_clockwheel_track", { slotId: slotId ?? null });

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
