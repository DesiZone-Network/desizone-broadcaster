const ALBUM_ART_BASE_URL_KEY = "dz-album-art-base-url";

export const DEFAULT_ALBUM_ART_BASE_URL =
  "https://dzblobstor1.blob.core.windows.net/albumart/pictures/";

function normalizeBaseUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) return DEFAULT_ALBUM_ART_BASE_URL;
  return trimmed.endsWith("/") ? trimmed : `${trimmed}/`;
}

export function getAlbumArtBaseUrl(): string {
  if (typeof window === "undefined") return DEFAULT_ALBUM_ART_BASE_URL;
  const stored = window.localStorage.getItem(ALBUM_ART_BASE_URL_KEY);
  return normalizeBaseUrl(stored ?? DEFAULT_ALBUM_ART_BASE_URL);
}

export function setAlbumArtBaseUrl(url: string): string {
  const normalized = normalizeBaseUrl(url);
  if (typeof window !== "undefined") {
    window.localStorage.setItem(ALBUM_ART_BASE_URL_KEY, normalized);
  }
  return normalized;
}

export function resolveAlbumArtUrl(
  picture: string | null | undefined,
  baseUrl?: string
): string | null {
  const raw = (picture ?? "").trim();
  if (!raw) return null;
  if (raw.startsWith("data:")) return raw;
  if (/^https?:\/\//i.test(raw)) return raw;
  if (raw.startsWith("file://")) return raw;

  const fileName = raw.split(/[\\/]/).pop() ?? raw;
  if (!fileName) return null;
  return `${normalizeBaseUrl(baseUrl ?? getAlbumArtBaseUrl())}${encodeURIComponent(fileName)}`;
}
