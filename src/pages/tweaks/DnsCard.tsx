import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { api, toAppError, type AdapterDns, type DnsPreset } from "@/lib/tauri";
import { cn } from "@/lib/utils";

const PRESETS: { value: DnsPreset; label: string; detail: string }[] = [
  { value: "automatic", label: "Automatic", detail: "From your router" },
  { value: "cloudflare", label: "Cloudflare", detail: "1.1.1.1" },
  { value: "google", label: "Google", detail: "8.8.8.8" },
  { value: "quad9", label: "Quad9", detail: "9.9.9.9, blocks malware" },
];

export function DnsCard() {
  const [adapters, setAdapters] = useState<AdapterDns[] | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .dnsStatus()
      .then(setAdapters)
      .catch((e) => toast.error(toAppError(e).message));
  }, []);

  const set = async (guid: string, preset: DnsPreset) => {
    setBusy(true);
    try {
      setAdapters(await api.dnsSet(guid, preset));
      toast.success(`DNS set to ${PRESETS.find((p) => p.value === preset)!.label}`);
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="rounded-lg border border-line bg-panel p-4">
      <h3 className="text-[17px] font-semibold">DNS servers</h3>
      <p className="mt-1 max-w-[72ch] text-[13px] text-muted">
        DNS turns names into addresses. A fast resolver makes websites and launchers connect quicker; it doesn't
        change in-game ping. Each change can be reverted.
      </p>
      {!adapters ? (
        <Skeleton className="mt-3 h-16 w-full" />
      ) : adapters.length === 0 ? (
        <p className="mt-3 text-muted">No physical network adapters found.</p>
      ) : (
        <ul className="mt-3 space-y-4">
          {adapters.map((a) => (
            <li key={a.guid}>
              <div className="flex flex-wrap items-center gap-2">
                <span>{a.name}</span>
                <Badge>{a.wired ? "Wired" : "Wi-Fi"}</Badge>
                {!a.connected && <span className="text-[12px] text-faint">Not connected</span>}
                <span className="text-[12px] text-faint">
                  {a.servers.length ? a.servers.join(", ") : "Automatic"}
                  {a.preset === null && " (custom)"}
                </span>
              </div>
              <div role="radiogroup" aria-label={`DNS for ${a.name}`} className="mt-2 flex flex-wrap gap-2">
                {PRESETS.map((p) => (
                  <button
                    key={p.value}
                    type="button"
                    role="radio"
                    aria-checked={a.preset === p.value}
                    disabled={busy}
                    onClick={() => a.preset !== p.value && void set(a.guid, p.value)}
                    className={cn(
                      "rounded-md border px-3 py-1.5 text-left text-sm transition-colors disabled:opacity-60",
                      a.preset === p.value ? "border-signal bg-signal-soft" : "border-line-strong hover:border-signal/60",
                    )}
                  >
                    <span className="block">{p.label}</span>
                    <span className="block text-[11px] text-faint">{p.detail}</span>
                  </button>
                ))}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
