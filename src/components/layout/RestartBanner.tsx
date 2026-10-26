import { RotateCcw } from "lucide-react";
import { restartAction } from "@/components/badges";
import { useAppStore } from "@/stores/app";

export function RestartBanner() {
  const pending = useAppStore((s) => s.pendingRestart);
  if (pending === "none") return null;
  return (
    <div role="status" className="flex items-center gap-3 border-b border-line bg-signal-soft px-10 py-2 text-[13px]">
      <RotateCcw className="size-4 shrink-0 text-signal" aria-hidden="true" />
      <span>Some changes finish taking effect after you {restartAction[pending].toLowerCase()}.</span>
    </div>
  );
}
