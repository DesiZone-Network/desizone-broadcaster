// Phase 7: Analytics & Operations bridge
import { invoke } from '@tauri-apps/api/core';

// ── Types ────────────────────────────────────────────────────────────────────

export interface TopSong {
  song_id: number;
  title: string;
  artist: string;
  play_count: number;
  total_played_ms: number;
}

export interface HeatmapData {
  date: string;
  hour: number;
  play_count: number;
  unique_songs: number;
}

export interface PlayHistoryEntry {
  id: number;
  song_id: number;
  title: string;
  artist: string;
  played_at: number;
  duration_ms: number;
  deck?: string;
}

export interface ListenerSnapshot {
  timestamp: number;
  listener_count: number;
  peak_listeners?: number;
}

export interface ListenerPeak {
  peak: number;
  average: number;
  timestamp: number;
}

export interface EventLogEntry {
  id: number;
  timestamp: number;
  level: string;
  category: string;
  event: string;
  message: string;
  metadata_json?: string;
  deck?: string;
  song_id?: number;
  encoder_id?: number;
}

export interface EventLogResponse {
  events: EventLogEntry[];
  total: number;
}

export interface SystemHealthSnapshot {
  timestamp: number;
  cpu_pct: number;
  memory_mb: number;
  ring_buffer_fill_deck_a: number;
  ring_buffer_fill_deck_b: number;
  decoder_latency_ms: number;
  stream_connected: boolean;
  mysql_connected: boolean;
  active_encoders: number;
}

export interface ReportData {
  report_type: string;
  generated_at: number;
  title: string;
  summary: ReportSummary;
  sections: ReportSection[];
}

export interface ReportSummary {
  total_plays?: number;
  total_listeners?: number;
  top_song?: string;
  peak_hour?: string;
}

export interface ReportSection {
  title: string;
  data: any;
}

export type ReportType =
  | { type: 'DailyBroadcast'; date: string }
  | { type: 'SongPlayHistory'; song_id: number; days: number }
  | { type: 'ListenerTrend'; period_days: number }
  | { type: 'RequestLog'; start_date: string; end_date: string }
  | { type: 'StreamUptime'; period_days: number };

// ── Play Stats ───────────────────────────────────────────────────────────────

export async function getTopSongs(period: string, limit: number): Promise<TopSong[]> {
  return invoke('get_top_songs', { period, limit });
}

export async function getHourlyHeatmap(
  startDate: string,
  endDate: string
): Promise<HeatmapData[]> {
  return invoke('get_hourly_heatmap', { startDate, endDate });
}

export async function getSongPlayHistory(
  songId: number,
  limit: number
): Promise<PlayHistoryEntry[]> {
  return invoke('get_song_play_history', { songId, limit });
}

// ── Listener Stats ───────────────────────────────────────────────────────────

export async function getListenerGraph(
  encoderId: number,
  period: string
): Promise<ListenerSnapshot[]> {
  return invoke('get_listener_graph', { encoderId, period });
}

export async function getListenerPeak(
  encoderId: number,
  period: string
): Promise<ListenerPeak> {
  return invoke('get_listener_peak', { encoderId, period });
}

// ── Event Log ────────────────────────────────────────────────────────────────

export async function getEventLog(params: {
  limit: number;
  offset: number;
  level?: string;
  category?: string;
  startTime?: number;
  endTime?: number;
  search?: string;
}): Promise<EventLogResponse> {
  return invoke('get_event_log', params);
}

export async function clearEventLog(olderThanDays: number): Promise<number> {
  return invoke('clear_event_log', { olderThanDays });
}

export async function writeEventLog(params: {
  level: 'debug' | 'info' | 'warn' | 'error';
  category: 'audio' | 'stream' | 'scheduler' | 'gateway' | 'scripting' | 'database' | 'system';
  event: string;
  message: string;
  deck?: string;
  songId?: number;
}): Promise<void> {
  return invoke('write_event_log', {
    level: params.level,
    category: params.category,
    event: params.event,
    message: params.message,
    deck: params.deck ?? null,
    songId: params.songId ?? null,
  });
}

// ── System Health ────────────────────────────────────────────────────────────

export async function getHealthSnapshot(): Promise<SystemHealthSnapshot> {
  return invoke('get_health_snapshot');
}

export async function getHealthHistory(periodMinutes: number): Promise<SystemHealthSnapshot[]> {
  return invoke('get_health_history', { periodMinutes });
}

// ── Reports ──────────────────────────────────────────────────────────────────

export async function generateReport(reportType: ReportType): Promise<ReportData> {
  return invoke('generate_report', { reportType });
}

export async function exportReportCsv(reportData: ReportData): Promise<string> {
  return invoke('export_report_csv', { reportData });
}

