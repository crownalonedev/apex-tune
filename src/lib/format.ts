/** Parses a WMI datetime ("yyyymmddHHMMSS.ffffff±UUU", UTC offset in minutes). */
export function parseWmiDate(value: string): Date | null {
  const m = /^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})\.\d+([+-])(\d{3})$/.exec(value);
  if (!m) return null;
  const [, y, mo, d, h, mi, s, sign, offset] = m;
  const utc = Date.UTC(Number(y), Number(mo) - 1, Number(d), Number(h), Number(mi), Number(s));
  const offsetMs = Number(offset) * 60_000 * (sign === "-" ? -1 : 1);
  return new Date(utc - offsetMs);
}

const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/** A WMI datetime for display, or the raw text if it can't be parsed. */
export function formatWmiDate(value: string): string {
  const date = parseWmiDate(value);
  return date ? dateTime.format(date) : value;
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

/** Bytes in binary units: "16 GB", "931 GB", "1.8 TB". */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 100 || unit === 0 ? 0 : 1;
  return `${value.toFixed(digits).replace(/\.0$/, "")} ${units[unit]}`;
}

/** Decimal units, the way drive makers label sizes: "1 TB", "500 GB". */
export function formatDriveSize(bytes: number): string {
  const tb = bytes / 1000 ** 4;
  if (tb >= 1) return `${Number(tb.toFixed(1))} TB`;
  return `${Math.round(bytes / 1000 ** 3)} GB`;
}
