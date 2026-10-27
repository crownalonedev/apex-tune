import { useState } from "react";
import { motion } from "framer-motion";
import { RestartBadge, RiskBadge } from "@/components/badges";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Switch } from "@/components/ui/switch";
import { api, type ChangePreview, type TweakInfo } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useTweaksStore } from "@/stores/tweaks";

const stateLabel = (t: TweakInfo): { text: string; tone: string } => {
  switch (t.state.state) {
    case "applied":
      return { text: "On", tone: "text-signal" };
    case "partial":
      return { text: "Partly on", tone: "text-moderate" };
    case "notApplied":
      return { text: "Off", tone: "text-faint" };
    case "notApplicable":
      return { text: t.state.reason, tone: "text-faint" };
    case "unknown":
      return { text: `Couldn't read: ${t.state.reason}`, tone: "text-danger" };
  }
};

/**
 * One tweak: what it does, its risk and restart needs, its current state,
 * and a switch. Moderate and Advanced tweaks confirm first, listing the
 * exact changes.
 */
export function TweakCard({ tweak }: { tweak: TweakInfo }) {
  const apply = useTweaksStore((s) => s.apply);
  const revert = useTweaksStore((s) => s.revert);
  const busy = useTweaksStore((s) => s.busy.has(tweak.id));
  const [confirming, setConfirming] = useState<ChangePreview[] | null>(null);

  // Partly applied shows as off: switching it on completes it.
  const on = tweak.state.state === "applied";
  const applicable = tweak.state.state !== "notApplicable" && tweak.state.state !== "unknown";
  // Turning off means reverting Apex Tune's own backup; without one there's nothing to restore.
  const canTurnOff = tweak.revertible;
  const disabled = busy || !applicable || (on && !canTurnOff);
  const label = stateLabel(tweak);

  const onChange = async (next: boolean) => {
    if (!next) {
      void revert(tweak.id);
      return;
    }
    if (tweak.risk === "safe") {
      void apply(tweak.id);
      return;
    }
    setConfirming(await api.tweakPlan(tweak.id).catch(() => []));
  };

  return (
    <motion.article
      layout="position"
      className={cn(
        "flex gap-4 rounded-lg border bg-panel p-4 transition-colors",
        on ? "border-signal/35" : "border-line",
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-[17px] font-semibold">{tweak.name}</h3>
          <RiskBadge risk={tweak.risk} />
          <RestartBadge restart={tweak.restart} />
          {tweak.recommended && <span className="text-[12px] text-faint">In the gaming preset</span>}
        </div>
        <p className="mt-1 max-w-[72ch] text-[13px] text-muted">{tweak.description}</p>
        <p className={cn("mt-2 text-[12px]", label.tone)}>
          {label.text}
          {on && !canTurnOff && " (already set before Apex Tune, nothing to revert)"}
        </p>
      </div>
      <div className="pt-1">
        <Switch
          checked={on}
          disabled={disabled}
          onCheckedChange={(v) => void onChange(v)}
          aria-label={`${tweak.name}: ${on ? "on" : "off"}`}
        />
      </div>

      <AlertDialog open={confirming !== null} onOpenChange={(o) => !o && setConfirming(null)}>
        <AlertDialogContent>
          <AlertDialogTitle>Turn on {tweak.name}?</AlertDialogTitle>
          <AlertDialogDescription>
            {tweak.risk === "advanced" ? "Advanced: " : "Moderate: "}
            {tweak.description}
          </AlertDialogDescription>
          {confirming && confirming.length > 0 && (
            <ul className="selectable mt-3 max-h-48 space-y-1.5 overflow-y-auto border-l border-line pl-3 text-[13px]">
              {confirming.map((c) => (
                <li key={c.target}>
                  <span className="block break-all text-muted">{c.target}</span>
                  <span className="text-faint">{c.from} becomes </span>
                  <span>{c.to}</span>
                </li>
              ))}
            </ul>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="primary" onClick={() => void apply(tweak.id)}>
              Turn on
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </motion.article>
  );
}
