import { useEffect } from "react";
import { toast } from "sonner";
import { PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { routeById } from "@/lib/routes";
import { APEX_PLAN_TWEAK, BALANCED_PLAN, type PlanInfo, type PowerOverview, type TweakInfo } from "@/lib/tauri";
import { usePowerStore } from "@/stores/power";
import { useTweaksStore } from "@/stores/tweaks";

export function PowerPage() {
  const route = routeById("power");
  const { overview, error, load } = usePowerStore();
  const loadTweaks = useTweaksStore((s) => s.load);
  const tweak = useTweaksStore((s) => s.overview?.tweaks.find((t) => t.id === APEX_PLAN_TWEAK));

  useEffect(() => {
    void load();
    void loadTweaks();
  }, [load, loadTweaks]);

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />
      {error && (
        <p role="alert" className="mt-4 max-w-[68ch] text-danger">
          {error.message}
        </p>
      )}
      {overview ? (
        <div className="mt-8 grid gap-x-10 gap-y-10 xl:grid-cols-[3fr_2fr]">
          <GamingPlanSection overview={overview} tweak={tweak} />
          <PlansSection overview={overview} />
        </div>
      ) : (
        <div className="mt-8 space-y-3" aria-busy="true">
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton key={i} className="h-6 w-full max-w-2xl" />
          ))}
        </div>
      )}
    </>
  );
}

function GamingPlanSection({ overview, tweak }: { overview: PowerOverview; tweak: TweakInfo | undefined }) {
  const { busy, install, remove, switchTo } = usePowerStore();
  const apex = overview.plans.find((p) => p.apex);
  const disabled = busy !== null;

  const status = !apex
    ? "Not installed."
    : apex.active
      ? "Installed and active."
      : "Installed, but another plan is active.";

  const onInstall = async () => {
    if (await install()) toast.success("Switched to Apex Tune Gaming");
  };
  const onRemove = async () => {
    if (await remove()) toast.success("Removed Apex Tune Gaming and switched back");
  };

  return (
    <section aria-labelledby="gaming-plan-heading">
      <h2 id="gaming-plan-heading" className="text-[19px] font-semibold">
        Apex Tune Gaming
      </h2>
      <p className="mt-1 max-w-[68ch] text-muted">
        A copy of Ultimate Performance (or High performance where that isn't available), tuned so the CPU and
        devices never drop into power saving while you play. Idle power use and temperatures go up.
        {overview.laptop && " Battery found, so this is the laptop variant: settings on battery stay power-saving."}
      </p>

      <div className="mt-4 flex flex-wrap items-center gap-3">
        <span className={apex?.active ? "text-signal" : undefined}>{status}</span>
        <span className="flex flex-wrap gap-2">
          {!apex && (
            <Button variant="primary" onClick={() => void onInstall()} disabled={disabled}>
              {busy === "install" ? "Installing…" : "Install and switch to it"}
            </Button>
          )}
          {apex && !apex.active && (
            <Button variant="primary" onClick={() => void switchTo(apex.guid)} disabled={disabled}>
              Switch to it
            </Button>
          )}
          {apex && tweak?.revertible && (
            <Button variant="ghost" onClick={() => void onRemove()} disabled={disabled}>
              {busy === "remove" ? "Removing…" : "Remove and switch back"}
            </Button>
          )}
        </span>
      </div>

      <table className="mt-5 w-full border-y border-line text-left">
        <caption className="sr-only">Settings in the Apex Tune Gaming plan</caption>
        <thead>
          <tr className="text-[13px] text-muted">
            <th scope="col" className="py-2 pr-4 font-normal">
              Setting
            </th>
            <th scope="col" className="py-2 pr-4 font-normal">
              Plugged in
            </th>
            {overview.laptop && (
              <th scope="col" className="py-2 pr-4 font-normal">
                On battery
              </th>
            )}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {overview.settings.map((row) => (
            <tr key={row.label}>
              <th scope="row" className="py-2.5 pr-4 font-normal">
                {row.label}
              </th>
              <td className="py-2.5 pr-4">
                <Value target={row.targetAc} current={row.currentAc} installed={!!apex} />
              </td>
              {overview.laptop && (
                <td className="py-2.5 pr-4">
                  <Value target={row.targetDc ?? "Unchanged"} current={row.currentDc} installed={!!apex && !!row.targetDc} />
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-2 text-[13px] text-faint">
        Core parking is hidden from Windows' own power settings; Apex Tune sets it directly in the plan.
      </p>
    </section>
  );
}

/** The planned value, plus the plan's current value when it has drifted. */
function Value({ target, current, installed }: { target: string; current: string | null; installed: boolean }) {
  if (!installed || current === null || current === target) return <span>{target}</span>;
  return (
    <span>
      {target}
      <span className="block text-[12px] text-moderate">Now {current}</span>
    </span>
  );
}

function PlansSection({ overview }: { overview: PowerOverview }) {
  const { busy, switchTo } = usePowerStore();
  const balanced = overview.plans.find((p) => p.guid === BALANCED_PLAN);

  return (
    <section aria-labelledby="plans-heading">
      <h2 id="plans-heading" className="text-[19px] font-semibold">
        Your power plans
      </h2>
      <ul className="mt-3 divide-y divide-line border-y border-line">
        {overview.plans.map((plan) => (
          <PlanRow
            key={plan.guid}
            plan={plan}
            busy={typeof busy === "object" && busy?.switching === plan.guid}
            disabled={busy !== null}
            onSwitch={() => void switchTo(plan.guid)}
          />
        ))}
      </ul>
      <div className="mt-3 text-[13px] text-muted">
        {balanced ? (
          !balanced.active && (
            <Button size="sm" variant="ghost" onClick={() => void switchTo(BALANCED_PLAN)} disabled={busy !== null}>
              Go back to Balanced
            </Button>
          )
        ) : (
          <p>
            The Balanced plan has been removed from this PC. To bring back Windows' default plans, run{" "}
            <code className="selectable rounded-sm bg-panel-raised px-1">powercfg -restoredefaultschemes</code> in
            an administrator terminal. That also removes custom plans, including Apex Tune Gaming.
          </p>
        )}
      </div>
    </section>
  );
}

function PlanRow({
  plan,
  busy,
  disabled,
  onSwitch,
}: {
  plan: PlanInfo;
  busy: boolean;
  disabled: boolean;
  onSwitch: () => void;
}) {
  return (
    <li className="flex items-center justify-between gap-3 py-2.5">
      <span className="flex min-w-0 flex-wrap items-center gap-2">
        <span className="break-words">{plan.name || "Unnamed plan"}</span>
        {plan.active && <Badge tone="signal">Active</Badge>}
        {plan.apex && <Badge>Apex Tune</Badge>}
        {plan.builtIn && <Badge>Windows</Badge>}
      </span>
      {!plan.active && (
        <Button size="sm" onClick={onSwitch} disabled={disabled}>
          {busy ? "Switching…" : "Switch"}
        </Button>
      )}
    </li>
  );
}
