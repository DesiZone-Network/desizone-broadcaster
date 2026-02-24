import React, { useState } from 'react';
import { EventLogPanel } from '../components/analytics/EventLogPanel';
import { SystemHealthPanel } from '../components/analytics/SystemHealthPanel';
import { TopSongsPanel } from '../components/analytics/TopSongsPanel';

export const AnalyticsPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'overview' | 'events' | 'health'>('overview');

  return (
    <div className="h-screen bg-gray-900 overflow-auto">
      <div className="p-6">
        <div className="mb-6">
          <h1 className="text-3xl font-bold text-white mb-2">Analytics & Operations</h1>
          <p className="text-gray-400">
            Monitor performance, view event logs, and analyze broadcast statistics
          </p>
        </div>

        {/* Tab Navigation */}
        <div className="flex gap-2 mb-6 border-b border-gray-700">
          <button
            onClick={() => setActiveTab('overview')}
            className={`px-4 py-2 font-medium transition-colors ${
              activeTab === 'overview'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            📊 Overview
          </button>
          <button
            onClick={() => setActiveTab('events')}
            className={`px-4 py-2 font-medium transition-colors ${
              activeTab === 'events'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            📋 Event Log
          </button>
          <button
            onClick={() => setActiveTab('health')}
            className={`px-4 py-2 font-medium transition-colors ${
              activeTab === 'health'
                ? 'text-blue-400 border-b-2 border-blue-400'
                : 'text-gray-400 hover:text-gray-300'
            }`}
          >
            💚 System Health
          </button>
        </div>

        {/* Tab Content */}
        {activeTab === 'overview' && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div>
              <SystemHealthPanel />
            </div>
            <div>
              <TopSongsPanel />
            </div>
            <div className="lg:col-span-2">
              <div className="p-4 bg-gray-800 rounded-lg">
                <h2 className="text-xl font-bold text-white mb-4">Quick Stats</h2>
                <div className="grid grid-cols-4 gap-4">
                  <div className="p-3 bg-gray-700 rounded text-center">
                    <div className="text-3xl font-bold text-white mb-1">0</div>
                    <div className="text-xs text-gray-400">Tracks Today</div>
                  </div>
                  <div className="p-3 bg-gray-700 rounded text-center">
                    <div className="text-3xl font-bold text-white mb-1">0</div>
                    <div className="text-xs text-gray-400">Avg Listeners</div>
                  </div>
                  <div className="p-3 bg-gray-700 rounded text-center">
                    <div className="text-3xl font-bold text-white mb-1">0</div>
                    <div className="text-xs text-gray-400">Requests</div>
                  </div>
                  <div className="p-3 bg-gray-700 rounded text-center">
                    <div className="text-3xl font-bold text-white mb-1">100%</div>
                    <div className="text-xs text-gray-400">Uptime</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'events' && (
          <div className="h-[calc(100vh-250px)]">
            <EventLogPanel />
          </div>
        )}

        {activeTab === 'health' && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div className="lg:col-span-2">
              <SystemHealthPanel />
            </div>
            <div className="lg:col-span-2">
              <div className="p-4 bg-gray-800 rounded-lg">
                <h2 className="text-xl font-bold text-white mb-4">System Information</h2>
                <div className="grid grid-cols-2 gap-4 text-sm">
                  <div>
                    <div className="text-gray-400 mb-1">Platform</div>
                    <div className="text-white">{navigator.platform}</div>
                  </div>
                  <div>
                    <div className="text-gray-400 mb-1">User Agent</div>
                    <div className="text-white truncate">{navigator.userAgent}</div>
                  </div>
                  <div>
                    <div className="text-gray-400 mb-1">Screen Resolution</div>
                    <div className="text-white">
                      {window.screen.width} × {window.screen.height}
                    </div>
                  </div>
                  <div>
                    <div className="text-gray-400 mb-1">Viewport</div>
                    <div className="text-white">
                      {window.innerWidth} × {window.innerHeight}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

