import React, { useState } from 'react';
import {
  startLiveTalk,
  stopLiveTalk,
  setMixMinus,
} from '../../lib/bridge6';

export const LiveTalkPanel: React.FC = () => {
  const [isLive, setIsLive] = useState(false);
  const [channel, setChannel] = useState('mic');
  const [mixMinusEnabled, setMixMinusEnabled] = useState(false);

  const handleStartLiveTalk = async () => {
    try {
      await startLiveTalk(channel);
      setIsLive(true);
    } catch (err) {
      alert(`Failed to start live talk: ${err}`);
    }
  };

  const handleStopLiveTalk = async () => {
    try {
      await stopLiveTalk();
      setIsLive(false);
    } catch (err) {
      alert(`Failed to stop live talk: ${err}`);
    }
  };

  const handleToggleMixMinus = async () => {
    try {
      const newValue = !mixMinusEnabled;
      await setMixMinus(newValue);
      setMixMinusEnabled(newValue);
    } catch (err) {
      alert(`Failed to toggle mix-minus: ${err}`);
    }
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg">
      <h2 className="text-xl font-bold mb-4 text-white">Live Talk</h2>

      <div className="space-y-4">
        <div>
          <label className="block text-sm text-gray-300 mb-2">Channel</label>
          <select
            value={channel}
            onChange={(e) => setChannel(e.target.value)}
            disabled={isLive}
            className="w-full px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 focus:border-blue-500 focus:outline-none disabled:opacity-50"
          >
            <option value="mic">Microphone</option>
            <option value="phone">Phone Line</option>
            <option value="skype">Skype/VoIP</option>
          </select>
        </div>

        <div className="flex items-center justify-between p-3 bg-gray-700 rounded">
          <div>
            <div className="text-white font-medium">Mix-Minus</div>
            <div className="text-xs text-gray-400">
              Prevents echo for remote callers
            </div>
          </div>
          <button
            onClick={handleToggleMixMinus}
            className={`px-3 py-1 rounded text-sm font-medium ${
              mixMinusEnabled
                ? 'bg-green-600 hover:bg-green-700'
                : 'bg-gray-600 hover:bg-gray-650'
            } text-white`}
          >
            {mixMinusEnabled ? 'ON' : 'OFF'}
          </button>
        </div>

        <div className="border-t border-gray-700 pt-4">
          {!isLive ? (
            <button
              onClick={handleStartLiveTalk}
              className="w-full px-4 py-3 bg-red-600 text-white rounded-lg font-bold hover:bg-red-700 transition-colors"
            >
              🎙️ GO LIVE
            </button>
          ) : (
            <div className="space-y-2">
              <div className="flex items-center justify-center gap-2 p-3 bg-red-900 rounded-lg animate-pulse">
                <div className="w-3 h-3 bg-red-500 rounded-full" />
                <span className="text-white font-bold">ON AIR</span>
              </div>
              <button
                onClick={handleStopLiveTalk}
                className="w-full px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700"
              >
                End Live Talk
              </button>
            </div>
          )}
        </div>

        <div className="text-xs text-gray-400 bg-gray-900 p-3 rounded">
          <strong>Note:</strong> Live talk mode routes your microphone directly to the air.
          Make sure your mic is configured properly before going live.
        </div>
      </div>
    </div>
  );
};

