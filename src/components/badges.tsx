import { Badge } from "@/components/ui/badge";
import type { Restart, Risk } from "@/lib/tauri";

const riskLabel: Record<Risk, string> = { safe: "Safe", moderate: "Moderate", advanced: "Advanced" };

export function RiskBadge({ risk }: { risk: Risk }) {
  return <Badge tone={risk}>{riskLabel[risk]}</Badge>;
}

export const restartLabel: Record<Exclude<Restart, "none">, string> = {
  explorer: "Explorer restart",
  signOut: "Sign out",
  reboot: "Restart",
};

export function RestartBadge({ restart }: { restart: Restart }) {
  if (restart === "none") return null;
  return <Badge tone="neutral">{restartLabel[restart]}</Badge>;
}

/** What to tell the user to do, as an imperative. */
export const restartAction: Record<Exclude<Restart, "none">, string> = {
  explorer: "Restart Explorer or sign out",
  signOut: "Sign out and back in",
  reboot: "Restart your PC",
};
