import React, { useEffect, useState } from 'react';
import {
  getRemoteSessions,
  kickRemoteDj,
  getRemoteDjPermissions,
  setRemoteDjPermissions,
  type RemoteSession,
  type DjPermissions,
} from '../../lib/bridge6';

export const RemoteDjManager: React.FC = () => {
  const [sessions, setSessions] = useState<RemoteSession[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [permissions, setPermissions] = useState<DjPermissions | null>(null);

  useEffect(() => {
    fetchSessions();
    const timer = setInterval(fetchSessions, 3000);
    return () => clearInterval(timer);
  }, []);

  const fetchSessions = async () => {
    try {
      const s = await getRemoteSessions();
      setSessions(s);
    } catch (err) {
      console.error('Failed to fetch sessions:', err);
    }
  };

  const handleSelectSession = async (sessionId: string) => {
    setSelectedSession(sessionId);
    try {
      const perms = await getRemoteDjPermissions(sessionId);
      setPermissions(perms);
    } catch (err) {
      console.error('Failed to fetch permissions:', err);
    }
  };

  const handleKick = async (sessionId: string) => {
    if (!confirm('Kick this remote DJ?')) return;
    try {
      await kickRemoteDj(sessionId);
      fetchSessions();
      if (selectedSession === sessionId) {
        setSelectedSession(null);
        setPermissions(null);
      }
    } catch (err) {
      alert(`Failed to kick: ${err}`);
    }
  };

  const handleUpdatePermissions = async () => {
    if (!selectedSession || !permissions) return;
    try {
      await setRemoteDjPermissions(selectedSession, permissions);
      alert('Permissions updated');
    } catch (err) {
      alert(`Failed to update permissions: ${err}`);
    }
  };

  const togglePermission = (key: keyof DjPermissions) => {
    if (!permissions) return;
    setPermissions({ ...permissions, [key]: !permissions[key] });
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg">
      <h2 className="text-xl font-bold mb-4 text-white">Remote DJ Sessions</h2>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <h3 className="text-sm font-semibold text-gray-300 mb-2">Active Sessions</h3>
          {sessions.length === 0 && (
            <div className="text-gray-500 text-sm">No remote DJs connected</div>
          )}
          <div className="space-y-2">
            {sessions.map((session) => (
              <div
                key={session.session_id}
                className={`p-3 rounded cursor-pointer ${
                  selectedSession === session.session_id
                    ? 'bg-blue-600'
                    : 'bg-gray-700 hover:bg-gray-650'
                }`}
                onClick={() => handleSelectSession(session.session_id)}
              >
                <div className="font-medium text-white">
                  {session.display_name || session.user_id}
                </div>
                <div className="text-xs text-gray-400">
                  Connected: {new Date(session.connected_at).toLocaleTimeString()}
                </div>
                <div className="text-xs text-gray-400">
                  Commands sent: {session.commands_sent}
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleKick(session.session_id);
                  }}
                  className="mt-2 px-2 py-1 bg-red-600 text-white text-xs rounded hover:bg-red-700"
                >
                  Kick
                </button>
              </div>
            ))}
          </div>
        </div>

        <div>
          <h3 className="text-sm font-semibold text-gray-300 mb-2">Permissions</h3>
          {!selectedSession && (
            <div className="text-gray-500 text-sm">Select a session to manage permissions</div>
          )}
          {selectedSession && permissions && (
            <div className="space-y-2">
              <PermissionToggle
                label="Load Tracks"
                checked={permissions.can_load_track}
                onChange={() => togglePermission('can_load_track')}
              />
              <PermissionToggle
                label="Play/Pause"
                checked={permissions.can_play_pause}
                onChange={() => togglePermission('can_play_pause')}
              />
              <PermissionToggle
                label="Seek"
                checked={permissions.can_seek}
                onChange={() => togglePermission('can_seek')}
              />
              <PermissionToggle
                label="Set Volume"
                checked={permissions.can_set_volume}
                onChange={() => togglePermission('can_set_volume')}
              />
              <PermissionToggle
                label="Add to Queue"
                checked={permissions.can_queue_add}
                onChange={() => togglePermission('can_queue_add')}
              />
              <PermissionToggle
                label="Remove from Queue"
                checked={permissions.can_queue_remove}
                onChange={() => togglePermission('can_queue_remove')}
              />
              <PermissionToggle
                label="Trigger Crossfade"
                checked={permissions.can_trigger_crossfade}
                onChange={() => togglePermission('can_trigger_crossfade')}
              />
              <PermissionToggle
                label="Set AutoPilot"
                checked={permissions.can_set_autopilot}
                onChange={() => togglePermission('can_set_autopilot')}
              />
              <button
                onClick={handleUpdatePermissions}
                className="w-full mt-4 px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700"
              >
                Update Permissions
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

const PermissionToggle: React.FC<{
  label: string;
  checked: boolean;
  onChange: () => void;
}> = ({ label, checked, onChange }) => (
  <label className="flex items-center justify-between p-2 bg-gray-700 rounded cursor-pointer hover:bg-gray-650">
    <span className="text-white text-sm">{label}</span>
    <input
      type="checkbox"
      checked={checked}
      onChange={onChange}
      className="w-4 h-4 text-blue-600 rounded focus:ring-blue-500"
    />
  </label>
);

