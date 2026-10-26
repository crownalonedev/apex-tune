import { useEffect } from "react";
import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatBytes, formatDriveSize } from "@/lib/format";
import type { DriveKind, HardwareSummary } from "@/lib/tauri";
import { useHardwareStore } from "@/stores/hardware";

const driveKind: Record<DriveKind, string> = {
  nvmeSsd: "NVMe SSD",
  ssd: "SSD",
  hdd: "Hard drive",
  unknown: "Drive",
};

export function HardwarePanel() {
  const { summary, error, load } = useHardwareStore();

  useEffect(() => {
    if (!summary) void load();
  }, [summary, load]);

  return (
    <section aria-labelledby="hardware-heading">
      <div className="flex items-center justify-between gap-3">
        <h2 id="hardware-heading" className="text-[19px] font-semibold">
          This PC
        </h2>
        <Button variant="ghost" size="icon" onClick={() => void load(true)} aria-label="Detect hardware again">
          <RefreshCw />
        </Button>
      </div>
      {error ? (
        <p role="alert" className="mt-3 text-danger">
          Couldn't detect hardware: {error.message}
        </p>
      ) : summary ? (
        <Details summary={summary} />
      ) : (
        <div className="mt-3 space-y-3" aria-busy="true">
          {Array.from({ length: 5 }, (_, i) => (
            <Skeleton key={i} className="h-5 w-full" />
          ))}
        </div>
      )}
    </section>
  );
}

function Details({ summary }: { summary: HardwareSummary }) {
  const { cpu, gpus, drives, volumes, windows } = summary;
  const ghz = cpu.baseMhz ? `${(cpu.baseMhz / 1000).toFixed(1)} GHz base` : null;
  const cores = cpu.physicalCores
    ? `${cpu.physicalCores} cores, ${cpu.logicalCores} threads`
    : `${cpu.logicalCores} threads`;
  const systemVolume = volumes.find((v) => /^c:/i.test(v.mount)) ?? volumes[0];

  return (
    <>
      <dl className="mt-3 divide-y divide-line border-y border-line">
        <Row label="Processor" lines={[cpu.name, [cores, ghz].filter(Boolean).join(", ")]} />
        <Row
          label="Graphics"
          lines={
            gpus.length === 0
              ? ["Not detected"]
              : gpus.flatMap((g) => [
                  g.name,
                  [g.vramBytes ? formatBytes(g.vramBytes) : null, `driver ${g.nvidiaDriver ?? g.driverVersion}`]
                    .filter(Boolean)
                    .join(", "),
                ])
          }
        />
        <Row label="Memory" lines={[formatBytes(summary.memoryBytes)]} />
        <Row
          label="Storage"
          lines={[
            ...drives.map((d) => `${d.name}, ${formatDriveSize(d.sizeBytes)} ${driveKind[d.kind]}`),
            ...(systemVolume
              ? [`${systemVolume.mount} has ${formatBytes(systemVolume.freeBytes)} free of ${formatBytes(systemVolume.totalBytes)}`]
              : []),
          ]}
        />
        <Row
          label="Windows"
          lines={[
            windows
              ? [windows.productName, windows.displayVersion].filter(Boolean).join(" ")
              : "Not detected",
            windows ? `Build ${windows.build}${windows.revision !== null ? `.${windows.revision}` : ""}` : "",
          ]}
        />
        <Row label="Type" lines={[summary.hasBattery ? "Laptop (battery found)" : "Desktop"]} />
      </dl>
      {summary.warnings.length > 0 && (
        <ul className="mt-2 space-y-0.5 text-[13px] text-moderate">
          {summary.warnings.map((w) => (
            <li key={w}>Couldn't read {w}</li>
          ))}
        </ul>
      )}
    </>
  );
}

function Row({ label, lines }: { label: string; lines: string[] }) {
  const [first, ...rest] = lines.filter(Boolean);
  return (
    <div className="flex gap-6 py-2.5">
      <dt className="w-24 shrink-0 text-muted">{label}</dt>
      <dd className="selectable min-w-0 flex-1">
        <span className="block break-words">{first}</span>
        {rest.map((line, i) => (
          <span key={i} className="block break-words text-[13px] text-muted">
            {line}
          </span>
        ))}
      </dd>
    </div>
  );
}
