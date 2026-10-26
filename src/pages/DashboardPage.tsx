import { PageHeader } from "@/components/layout/PageHeader";
import { Skeleton } from "@/components/ui/skeleton";
import { routeById } from "@/lib/routes";
import { useAppStore } from "@/stores/app";

export function DashboardPage() {
  const route = routeById("dashboard");
  const info = useAppStore((s) => s.info);
  const error = useAppStore((s) => s.infoError);

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />

      <section aria-labelledby="system-heading" className="mt-8 max-w-2xl">
        <h2 id="system-heading" className="text-[19px] font-semibold">
          This system
        </h2>
        {error ? (
          <p role="alert" className="mt-3 text-danger">
            Couldn't read system details: {error.message}
          </p>
        ) : (
          <dl className="mt-3 divide-y divide-line border-y border-line">
            <Row label="Windows" value={info?.osName} />
            <Row label="Kernel build" value={info?.osBuild || (info ? "Unknown" : undefined)} />
            <Row label="Apex Tune" value={info ? `Version ${info.version}` : undefined} />
            <Row label="Backups and logs" value={info?.dataDir || (info ? "Not available in preview" : undefined)} />
          </dl>
        )}
        <p className="mt-4 text-[13px] text-faint">
          Hardware, live usage and the Optimization Score arrive in build stage 3.
        </p>
      </section>
    </>
  );
}

function Row({ label, value }: { label: string; value: string | undefined }) {
  return (
    <div className="flex gap-6 py-2.5">
      <dt className="w-40 shrink-0 text-muted">{label}</dt>
      <dd className="selectable min-w-0 flex-1 break-words">
        {value === undefined ? <Skeleton className="h-4 w-56" /> : value}
      </dd>
    </div>
  );
}
