import type { SamSong } from "./bridge";

export interface SongDragPayload {
  version: 1;
  source: "library" | "queue" | "requests" | "unknown";
  song: Partial<SamSong>;
}

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
    const song =
      maybePayload.version === 1 && maybePayload.song
        ? maybePayload.song
        : (parsed as Partial<SamSong>);

    if (!song || typeof song !== "object") return null;
    if (typeof song.filename !== "string" || song.filename.trim().length === 0) {
      return null;
    }

    return song;
  } catch {
    return null;
  }
}
