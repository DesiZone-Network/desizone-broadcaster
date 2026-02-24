import React, { useEffect, useState } from 'react';
import { getTopSongs, type TopSong } from '../../lib/bridge7';

export const TopSongsPanel: React.FC = () => {
  const [songs, setSongs] = useState<TopSong[]>([]);
  const [period, setPeriod] = useState('7d');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    fetchTopSongs();
  }, [period]);

  const fetchTopSongs = async () => {
    setLoading(true);
    try {
      const result = await getTopSongs(period, 20);
      setSongs(result);
    } catch (err) {
      console.error('Failed to fetch top songs:', err);
    } finally {
      setLoading(false);
    }
  };

  const formatDuration = (ms: number) => {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    if (hours > 0) {
      return `${hours}h ${minutes % 60}m`;
    }
    return `${minutes}m ${seconds % 60}s`;
  };

  return (
    <div className="p-4 bg-gray-800 rounded-lg">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold text-white">Top Songs</h2>
        <select
          value={period}
          onChange={(e) => setPeriod(e.target.value)}
          className="px-3 py-2 bg-gray-700 text-white rounded border border-gray-600 text-sm"
        >
          <option value="7d">Last 7 Days</option>
          <option value="30d">Last 30 Days</option>
          <option value="90d">Last 90 Days</option>
          <option value="all">All Time</option>
        </select>
      </div>

      {loading && <div className="text-gray-400">Loading...</div>}

      {!loading && songs.length === 0 && (
        <div className="text-gray-400 text-center py-8">No data available</div>
      )}

      {!loading && songs.length > 0 && (
        <div className="space-y-2">
          {songs.map((song, index) => (
            <div
              key={song.song_id}
              className="flex items-center gap-3 p-3 bg-gray-700 rounded hover:bg-gray-650"
            >
              <div
                className={`flex-shrink-0 w-8 h-8 flex items-center justify-center rounded-full font-bold ${
                  index === 0
                    ? 'bg-yellow-500 text-yellow-900'
                    : index === 1
                    ? 'bg-gray-400 text-gray-900'
                    : index === 2
                    ? 'bg-orange-600 text-orange-100'
                    : 'bg-gray-600 text-gray-300'
                }`}
              >
                {index + 1}
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-white font-medium truncate">{song.title}</div>
                <div className="text-sm text-gray-400 truncate">{song.artist}</div>
              </div>
              <div className="flex-shrink-0 text-right">
                <div className="text-white font-semibold">{song.play_count} plays</div>
                <div className="text-xs text-gray-400">
                  {formatDuration(song.total_played_ms)}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

