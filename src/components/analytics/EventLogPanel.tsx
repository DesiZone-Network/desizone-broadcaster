import React, { useEffect, useState } from 'react';
import { getEventLog, clearEventLog, type EventLogEntry } from '../../lib/bridge7';

export const EventLogPanel: React.FC = () => {
  const [events, setEvents] = useState<EventLogEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [levelFilter, setLevelFilter] = useState<string>('');
  const [categoryFilter, setCategoryFilter] = useState<string>('');
  const [search, setSearch] = useState('');
  const pageSize = 50;

  useEffect(() => {
    fetchEvents();
  }, [page, levelFilter, categoryFilter, search]);

  const fetchEvents = async () => {
    try {
      const result = await getEventLog({
        limit: pageSize,
        offset: page * pageSize,
        level: levelFilter || undefined,
        category: categoryFilter || undefined,
        search: search || undefined,
      });
      setEvents(result.events);
      setTotal(result.total);
    } catch (err) {
      console.error('Failed to fetch events:', err);
    }
  };

  const handleClear = async () => {
    if (!confirm('Clear events older than 30 days?')) return;
    try {
      const deleted = await clearEventLog(30);
      alert(`Cleared ${deleted} old events`);
      fetchEvents();
    } catch (err) {
      alert(`Failed to clear events: ${err}`);
    }
  };

  const getLevelColor = (level: string) => {
    switch (level) {
      case 'error':
        return 'text-red-400';
      case 'warn':
        return 'text-yellow-400';
      case 'info':
        return 'text-blue-400';
      case 'debug':
        return 'text-gray-400';
      default:
        return 'text-gray-300';
    }
  };

  const getLevelIcon = (level: string) => {
    switch (level) {
      case 'error':
        return '🔴';
      case 'warn':
        return '🟡';
      case 'info':
        return '🟢';
      case 'debug':
        return '⚪';
      default:
        return '⚪';
    }
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg h-full flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold text-white">Event Log</h2>
        <button
          onClick={handleClear}
          className="px-3 py-1 bg-red-600 text-white text-sm rounded hover:bg-red-700"
        >
          Clear Old
        </button>
      </div>

      {/* Filters */}
      <div className="flex gap-2 mb-4">
        <select
          value={levelFilter}
          onChange={(e) => setLevelFilter(e.target.value)}
          className="px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 text-sm"
        >
          <option value="">All Levels</option>
          <option value="error">Error</option>
          <option value="warn">Warning</option>
          <option value="info">Info</option>
          <option value="debug">Debug</option>
        </select>

        <select
          value={categoryFilter}
          onChange={(e) => setCategoryFilter(e.target.value)}
          className="px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 text-sm"
        >
          <option value="">All Categories</option>
          <option value="audio">Audio</option>
          <option value="stream">Stream</option>
          <option value="scheduler">Scheduler</option>
          <option value="gateway">Gateway</option>
          <option value="scripting">Scripting</option>
          <option value="database">Database</option>
          <option value="system">System</option>
        </select>

        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search..."
          className="flex-1 px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 text-sm"
        />
      </div>

      {/* Event table */}
      <div className="flex-1 overflow-auto bg-gray-900 rounded">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-gray-800 border-b border-gray-700">
            <tr>
              <th className="px-3 py-2 text-left text-gray-400 font-medium">Time</th>
              <th className="px-3 py-2 text-left text-gray-400 font-medium">Level</th>
              <th className="px-3 py-2 text-left text-gray-400 font-medium">Category</th>
              <th className="px-3 py-2 text-left text-gray-400 font-medium">Event</th>
              <th className="px-3 py-2 text-left text-gray-400 font-medium">Message</th>
            </tr>
          </thead>
          <tbody>
            {events.map((event) => (
              <tr key={event.id} className="border-b border-gray-800 hover:bg-gray-800">
                <td className="px-3 py-2 text-gray-400 whitespace-nowrap">
                  {new Date(event.timestamp).toLocaleTimeString()}
                </td>
                <td className={`px-3 py-2 whitespace-nowrap ${getLevelColor(event.level)}`}>
                  {getLevelIcon(event.level)} {event.level}
                </td>
                <td className="px-3 py-2 text-gray-300 whitespace-nowrap">{event.category}</td>
                <td className="px-3 py-2 text-gray-300 whitespace-nowrap">{event.event}</td>
                <td className="px-3 py-2 text-gray-300">{event.message}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex items-center justify-between mt-4 text-sm">
        <div className="text-gray-400">
          Showing {page * pageSize + 1}-{Math.min((page + 1) * pageSize, total)} of {total}
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setPage(Math.max(0, page - 1))}
            disabled={page === 0}
            className="px-3 py-1 bg-gray-700 text-white rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Previous
          </button>
          <button
            onClick={() => setPage(page + 1)}
            disabled={(page + 1) * pageSize >= total}
            className="px-3 py-1 bg-gray-700 text-white rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Next
          </button>
        </div>
      </div>
    </div>
  );
};

