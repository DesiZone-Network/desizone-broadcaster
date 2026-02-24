import React, { useEffect, useState } from 'react';
import {
  connectGateway,
  disconnectGateway,
  getGatewayStatus,
  type GatewayStatus,
} from '../../lib/bridge6';

export const GatewayConnectionPanel: React.FC = () => {
  const [status, setStatus] = useState<GatewayStatus>({
    connected: false,
    url: '',
    reconnecting: false,
  });
  const [url, setUrl] = useState('wss://gateway.desizone.network');
  const [token, setToken] = useState('');
  const [connecting, setConnecting] = useState(false);

  useEffect(() => {
    const fetchStatus = async () => {
      const s = await getGatewayStatus();
      setStatus(s);
    };
    fetchStatus();
    const timer = setInterval(fetchStatus, 2000);
    return () => clearInterval(timer);
  }, []);

  const handleConnect = async () => {
    if (!url || !token) {
      alert('Please enter both URL and token');
      return;
    }
    setConnecting(true);
    try {
      const newStatus = await connectGateway(url, token);
      setStatus(newStatus);
      if (newStatus.connected) {
        alert('Connected to gateway!');
      }
    } catch (err) {
      alert(`Connection failed: ${err}`);
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnectGateway();
      setStatus({ ...status, connected: false });
    } catch (err) {
      console.error('Disconnect error:', err);
    }
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg">
      <h2 className="text-xl font-bold mb-4 text-white">Gateway Connection</h2>

      <div className="mb-4">
        <div className="flex items-center gap-2 mb-2">
          <div
            className={`w-3 h-3 rounded-full ${
              status.connected ? 'bg-green-500' : 'bg-red-500'
            }`}
          />
          <span className="text-white font-medium">
            {status.connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
        {status.reconnecting && (
          <div className="text-yellow-400 text-sm">Reconnecting...</div>
        )}
        {status.last_error && (
          <div className="text-red-400 text-sm">Error: {status.last_error}</div>
        )}
      </div>

      {!status.connected && (
        <div className="space-y-3">
          <div>
            <label className="block text-sm text-gray-300 mb-1">Gateway URL</label>
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="wss://gateway.desizone.network"
              className="w-full px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
            />
          </div>
          <div>
            <label className="block text-sm text-gray-300 mb-1">Auth Token</label>
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="Enter your gateway token"
              className="w-full px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
            />
          </div>
          <button
            onClick={handleConnect}
            disabled={connecting}
            className="w-full px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {connecting ? 'Connecting...' : 'Connect'}
          </button>
        </div>
      )}

      {status.connected && (
        <div className="space-y-3">
          <div className="text-sm text-gray-300">
            <div>URL: {status.url}</div>
          </div>
          <button
            onClick={handleDisconnect}
            className="w-full px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
          >
            Disconnect
          </button>
        </div>
      )}
    </div>
  );
};

