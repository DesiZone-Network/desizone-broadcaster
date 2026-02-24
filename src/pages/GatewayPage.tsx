import React from 'react';
import { GatewayConnectionPanel } from '../components/gateway/GatewayConnectionPanel';
import { RemoteDjManager } from '../components/gateway/RemoteDjManager';
import { AutoPilotPanel } from '../components/gateway/AutoPilotPanel';
import { LiveTalkPanel } from '../components/gateway/LiveTalkPanel';

export const GatewayPage: React.FC = () => {
  return (
    <div className="h-screen bg-gray-900 overflow-auto">
      <div className="p-6">
        <div className="mb-6">
          <h1 className="text-3xl font-bold text-white mb-2">DBE Gateway</h1>
          <p className="text-gray-400">
            Connect to the DesiZone Broadcasting Engine for remote control and cloud sync
          </p>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Connection Panel */}
          <div>
            <GatewayConnectionPanel />
          </div>

          {/* AutoPilot Panel */}
          <div>
            <AutoPilotPanel />
          </div>

          {/* Remote DJ Manager */}
          <div className="lg:col-span-2">
            <RemoteDjManager />
          </div>

          {/* Live Talk Panel */}
          <div>
            <LiveTalkPanel />
          </div>

          {/* Status & Info Panel */}
          <div className="p-4 bg-gray-800 rounded-lg">
            <h2 className="text-xl font-bold mb-4 text-white">Gateway Features</h2>
            <div className="space-y-3 text-sm">
              <div className="flex items-start gap-3">
                <div className="text-green-400 mt-0.5">✓</div>
                <div>
                  <div className="text-white font-medium">Real-time State Sync</div>
                  <div className="text-gray-400">
                    Queue, now playing, and deck states are synced to the cloud
                  </div>
                </div>
              </div>
              <div className="flex items-start gap-3">
                <div className="text-green-400 mt-0.5">✓</div>
                <div>
                  <div className="text-white font-medium">Remote DJ Control</div>
                  <div className="text-gray-400">
                    Allow authorized users to control the broadcaster remotely
                  </div>
                </div>
              </div>
              <div className="flex items-start gap-3">
                <div className="text-green-400 mt-0.5">✓</div>
                <div>
                  <div className="text-white font-medium">Song Requests</div>
                  <div className="text-gray-400">
                    Receive and manage listener song requests from the web
                  </div>
                </div>
              </div>
              <div className="flex items-start gap-3">
                <div className="text-green-400 mt-0.5">✓</div>
                <div>
                  <div className="text-white font-medium">AutoPilot Mode</div>
                  <div className="text-gray-400">
                    Let the system manage playback based on rotation rules
                  </div>
                </div>
              </div>
              <div className="flex items-start gap-3">
                <div className="text-green-400 mt-0.5">✓</div>
                <div>
                  <div className="text-white font-medium">Live Talk Integration</div>
                  <div className="text-gray-400">
                    Go live with microphone input and mix-minus support
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="mt-6 p-4 bg-blue-900 bg-opacity-30 border border-blue-700 rounded-lg">
          <div className="flex items-start gap-3">
            <div className="text-blue-400 text-xl">ℹ️</div>
            <div>
              <div className="text-white font-medium mb-1">Getting Started</div>
              <div className="text-gray-300 text-sm">
                To connect to the gateway, you'll need a valid authentication token from your
                DesiZone network administrator. Once connected, you can enable AutoPilot mode
                or allow remote DJs to control the broadcaster from anywhere.
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

