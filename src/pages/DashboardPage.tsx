import { useEffect } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { SafetyPanel } from "@/components/SafetyPanel";
import { HardwarePanel } from "@/components/dashboard/HardwarePanel";
import { LiveStats } from "@/components/dashboard/LiveStats";
import { OptimizeDialog } from "@/components/dashboard/OptimizeDialog";
import { ScoreDial } from "@/components/dashboard/ScoreDial";
import { routeById } from "@/lib/routes";
import { useTweaksStore } from "@/stores/tweaks";

export function DashboardPage() {
  const route = routeById("dashboard");
  const overview = useTweaksStore((s) => s.overview);
  const overviewError = useTweaksStore((s) => s.error);
  const loadOverview = useTweaksStore((s) => s.load);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} actions={<OptimizeDialog />} />

      <div className="mt-8 grid gap-x-10 gap-y-8 lg:grid-cols-[minmax(240px,320px)_1fr]">
        <div>
          <ScoreDial score={overview?.score ?? null} />
          {overviewError && (
            <p role="alert" className="mt-2 text-center text-[13px] text-danger">
              {overviewError.message}
            </p>
          )}
        </div>
        <LiveStats />
      </div>

      <div className="mt-10 grid gap-x-10 gap-y-10 xl:grid-cols-2">
        <HardwarePanel />
        <SafetyPanel />
      </div>
    </>
  );
}
