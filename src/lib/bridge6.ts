// Phase 6: Gateway integration bridge
import { invoke } from '@tauri-apps/api/core';

export interface GatewayStatus {
  connected: boolean;
  url: string;
  reconnecting: boolean;
  last_error?: string;
}

export interface AutoPilotStatus {
  enabled: boolean;
  mode: 'rotation' | 'queue' | 'scheduled';
  current_rule?: string;
}

export interface RemoteSession {
  session_id: string;
  user_id: string;
  display_name?: string;
  connected_at: number;
  commands_sent: number;
}

export interface DjPermissions {
  can_load_track: boolean;
  can_play_pause: boolean;
  can_seek: boolean;
  can_set_volume: boolean;
  can_queue_add: boolean;
  can_queue_remove: boolean;
  can_trigger_crossfade: boolean;
  can_set_autopilot: boolean;
}

// Gateway connection
export async function connectGateway(url: string, token: string): Promise<GatewayStatus> {
  return invoke('connect_gateway', { url, token });
}

export async function disconnectGateway(): Promise<void> {
  return invoke('disconnect_gateway');
}

export async function getGatewayStatus(): Promise<GatewayStatus> {
  return invoke('get_gateway_status');
}

// AutoPilot
export async function setAutoPilot(enabled: boolean, mode: string): Promise<void> {
  return invoke('set_autopilot', { enabled, mode });
}

export async function getAutoPilotStatus(): Promise<AutoPilotStatus> {
  return invoke('get_autopilot_status');
}

// Remote DJ sessions
export async function getRemoteSessions(): Promise<RemoteSession[]> {
  return invoke('get_remote_sessions');
}

export async function kickRemoteDj(sessionId: string): Promise<void> {
  return invoke('kick_remote_dj', { sessionId });
}

export async function setRemoteDjPermissions(
  sessionId: string,
  permissions: DjPermissions
): Promise<void> {
  return invoke('set_remote_dj_permissions', { sessionId, permissions });
}

export async function getRemoteDjPermissions(sessionId: string): Promise<DjPermissions> {
  return invoke('get_remote_dj_permissions', { sessionId });
}

// Live talk
export async function startLiveTalk(channel: string): Promise<void> {
  return invoke('start_live_talk', { channel });
}

export async function stopLiveTalk(): Promise<void> {
  return invoke('stop_live_talk');
}

export async function setMixMinus(enabled: boolean): Promise<void> {
  return invoke('set_mix_minus', { enabled });
}

