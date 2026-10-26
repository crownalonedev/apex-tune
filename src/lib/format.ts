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
