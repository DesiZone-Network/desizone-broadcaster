import React, { useEffect, useState } from 'react';
import {
  getAutoPilotStatus,
  setAutoPilot,
  type AutoPilotStatus,
} from '../../lib/bridge6';

export const AutoPilotPanel: React.FC = () => {
  const [status, setStatus] = useState<AutoPilotStatus>({
    enabled: false,
    mode: 'rotation',
  });

  useEffect(() => {
    fetchStatus();
  }, []);

  const fetchStatus = async () => {
    try {
      const s = await getAutoPilotStatus();
      setStatus(s);
    } catch (err) {
      console.error('Failed to fetch autopilot status:', err);
    }
  };

  const handleToggle = async () => {
    try {
      await setAutoPilot(!status.enabled, status.mode);
      fetchStatus();
    } catch (err) {
      alert(`Failed to toggle autopilot: ${err}`);
    }
  };

  const handleModeChange = async (mode: 'rotation' | 'queue' | 'scheduled') => {
    try {
      await setAutoPilot(status.enabled, mode);
      fetchStatus();
    } catch (err) {
      alert(`Failed to change mode: ${err}`);
    }
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold text-white">AutoPilot</h2>
        <button
          onClick={handleToggle}
          className={`px-4 py-2 rounded font-medium ${
            status.enabled
              ? 'bg-green-600 hover:bg-green-700'
              : 'bg-gray-600 hover:bg-gray-700'
          } text-white`}
        >
          {status.enabled ? 'ON' : 'OFF'}
        </button>
      </div>

      <div className="space-y-3">
        <div>
          <label className="block text-sm text-gray-300 mb-2">Mode</label>
          <div className="grid grid-cols-3 gap-2">
            <button
              onClick={() => handleModeChange('rotation')}
              className={`px-3 py-2 rounded text-sm ${
                status.mode === 'rotation'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-650'
              }`}
            >
              Rotation
            </button>
            <button
              onClick={() => handleModeChange('queue')}
              className={`px-3 py-2 rounded text-sm ${
                status.mode === 'queue'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-650'
              }`}
            >
              Queue
            </button>
            <button
              onClick={() => handleModeChange('scheduled')}
              className={`px-3 py-2 rounded text-sm ${
                status.mode === 'scheduled'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-650'
              }`}
            >
              Scheduled
            </button>
          </div>
        </div>

        {status.current_rule && (
          <div className="p-3 bg-gray-700 rounded">
            <div className="text-xs text-gray-400 mb-1">Current Rule</div>
            <div className="text-white text-sm">{status.current_rule}</div>
          </div>
        )}

        <div className="text-xs text-gray-400 mt-4">
          {status.enabled ? (
            <div className="text-green-400">
              ✓ AutoPilot is managing playback ({status.mode} mode)
            </div>
          ) : (
            <div className="text-gray-500">
              AutoPilot is disabled. Enable to let the system manage playback automatically.
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

