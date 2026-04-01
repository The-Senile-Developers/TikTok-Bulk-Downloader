export function extractUsernameFromUrl(url: string) {
  const match = url.trim().match(/tiktok\.com\/@([^/?#]+)/i);
  return match?.[1] || null;
}

export function isTikTokProfileUrl(url: string) {
  return /tiktok\.com\/@[^/?#]+\/?$/i.test(url.trim());
}

export function isTikTokVideoUrl(url: string) {
  return /tiktok\.com\/@[^/]+\/video\/\d+/i.test(url.trim());
}

export function sanitizePathSegment(value: string) {
  const sanitized = value
    .replace(/[<>:"/\\|?*]/g, "_")
    .trim()
    .replace(/\.+$/, "");

  return sanitized || "unknown";
}
