import React, { useEffect, useState } from 'react';
import { getHealthSnapshot, type SystemHealthSnapshot } from '../../lib/bridge7';

export const SystemHealthPanel: React.FC = () => {
  const [health, setHealth] = useState<SystemHealthSnapshot | null>(null);

  useEffect(() => {
    fetchHealth();
    const timer = setInterval(fetchHealth, 5000);
    return () => clearInterval(timer);
  }, []);

  const fetchHealth = async () => {
    try {
      const snapshot = await getHealthSnapshot();
      setHealth(snapshot);
    } catch (err) {
      console.error('Failed to fetch health:', err);
    }
  };

  if (!health) {
    return <div className="p-4 bg-gray-800 rounded-lg text-gray-400">Loading...</div>;
  }

  const getBufferColor = (fill: number) => {
    if (fill >= 0.5) return 'bg-green-500';
    if (fill >= 0.2) return 'bg-yellow-500';
    return 'bg-red-500';
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg">
      <h2 className="text-xl font-bold text-white mb-4">System Health</h2>

      <div className="grid grid-cols-2 gap-4">
        {/* CPU Usage */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">CPU Usage</div>
          <div className="text-2xl font-bold text-white">{health.cpu_pct.toFixed(1)}%</div>
          <div className="w-full bg-gray-600 rounded-full h-2 mt-2">
            <div
              className="bg-blue-500 h-2 rounded-full transition-all"
              style={{ width: `${Math.min(100, health.cpu_pct)}%` }}
            />
          </div>
        </div>

        {/* Memory Usage */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">Memory Usage</div>
          <div className="text-2xl font-bold text-white">{health.memory_mb.toFixed(0)} MB</div>
          <div className="text-xs text-gray-400 mt-1">
            {(health.memory_mb / 1024).toFixed(2)} GB
          </div>
        </div>

        {/* Ring Buffer Deck A */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">Buffer Deck A</div>
          <div className="text-2xl font-bold text-white">
            {(health.ring_buffer_fill_deck_a * 100).toFixed(0)}%
          </div>
          <div className="w-full bg-gray-600 rounded-full h-2 mt-2">
            <div
              className={`h-2 rounded-full transition-all ${getBufferColor(
                health.ring_buffer_fill_deck_a
              )}`}
              style={{ width: `${health.ring_buffer_fill_deck_a * 100}%` }}
            />
          </div>
          {health.ring_buffer_fill_deck_a < 0.2 && (
            <div className="text-xs text-red-400 mt-1">⚠️ Low buffer warning</div>
          )}
        </div>

        {/* Ring Buffer Deck B */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">Buffer Deck B</div>
          <div className="text-2xl font-bold text-white">
            {(health.ring_buffer_fill_deck_b * 100).toFixed(0)}%
          </div>
          <div className="w-full bg-gray-600 rounded-full h-2 mt-2">
            <div
              className={`h-2 rounded-full transition-all ${getBufferColor(
                health.ring_buffer_fill_deck_b
              )}`}
              style={{ width: `${health.ring_buffer_fill_deck_b * 100}%` }}
            />
          </div>
          {health.ring_buffer_fill_deck_b < 0.2 && (
            <div className="text-xs text-red-400 mt-1">⚠️ Low buffer warning</div>
          )}
        </div>

        {/* Decoder Latency */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">Decoder Latency</div>
          <div className="text-2xl font-bold text-white">
            {health.decoder_latency_ms.toFixed(1)} ms
          </div>
          {health.decoder_latency_ms > 10 && (
            <div className="text-xs text-yellow-400 mt-1">High latency</div>
          )}
        </div>

        {/* Active Encoders */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">Active Encoders</div>
          <div className="text-2xl font-bold text-white">{health.active_encoders}</div>
        </div>

        {/* Stream Status */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">Stream Status</div>
          <div className="flex items-center gap-2">
            <div
              className={`w-3 h-3 rounded-full ${
                health.stream_connected ? 'bg-green-500' : 'bg-red-500'
              }`}
            />
            <span className="text-white font-medium">
              {health.stream_connected ? 'Connected' : 'Disconnected'}
            </span>
          </div>
        </div>

        {/* MySQL Status */}
        <div className="p-3 bg-gray-700 rounded">
          <div className="text-xs text-gray-400 mb-1">SAM Database</div>
          <div className="flex items-center gap-2">
            <div
              className={`w-3 h-3 rounded-full ${
                health.mysql_connected ? 'bg-green-500' : 'bg-red-500'
              }`}
            />
            <span className="text-white font-medium">
              {health.mysql_connected ? 'Connected' : 'Disconnected'}
            </span>
          </div>
        </div>
      </div>

      <div className="mt-4 text-xs text-gray-400">
        Last updated: {new Date(health.timestamp).toLocaleTimeString()}
      </div>
    </div>
  );
};

