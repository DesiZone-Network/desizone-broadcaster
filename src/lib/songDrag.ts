import type { SamSong } from "./bridge";

export interface SongDragPayload {
  version: 1;
  source: "library" | "queue" | "requests" | "unknown";
  song: Partial<SamSong>;
}

const APP_DRAG_SOURCES = new Set<SongDragPayload["source"]>([
  "library",
  "queue",
  "requests",
]);

export function serializeSongDragPayload(
  song: Partial<SamSong>,
  source: SongDragPayload["source"]
): string {
  return JSON.stringify({ version: 1, source, song } satisfies SongDragPayload);
}

export function parseSongDragPayload(raw: string): Partial<SamSong> | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") return null;

    const maybePayload = parsed as SongDragPayload;
    if (maybePayload.version !== 1 || !APP_DRAG_SOURCES.has(maybePayload.source)) {
      return null;
    }
    const song = maybePayload.song;

    if (!song || typeof song !== "object") return null;
    if (typeof song.filename !== "string" || song.filename.trim().length === 0) {
      return null;
    }

    return song;
  } catch {
    return null;
  }
}

export function parseSongDragFromDataTransfer(dataTransfer: DataTransfer): {
  song: Partial<SamSong> | null;
  error: string | null;
} {
  const raw =
    dataTransfer.getData("text/plain") ||
    dataTransfer.getData("application/json");
  if (!raw) {
    return {
      song: null,
      error: "No drag payload found. Drag tracks from Library, Queue, or Requests.",
    };
  }

  const song = parseSongDragPayload(raw);
  if (!song) {
    return {
      song: null,
      error: "Unsupported drag payload. Drag tracks from Library, Queue, or Requests.",
    };
  }
  return { song, error: null };
}
