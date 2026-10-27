import type { PageId } from "@/lib/routes";
import { routeById } from "@/lib/routes";
import { DashboardPage } from "./DashboardPage";
import { DebloatPage } from "./DebloatPage";
import { NvidiaPage } from "./NvidiaPage";
import { PowerPage } from "./PowerPage";
import { TimerPage } from "./TimerPage";
import { TweaksPage } from "./TweaksPage";
import { PlannedPage } from "./PlannedPage";

const planned: Record<Exclude<PageId, "dashboard" | "power" | "timer" | "debloat" | "tweaks" | "nvidia">, { stage: number; items: string[] }> = {
  profiles: {
    stage: 9,
    items: ["Save, load, import and export profiles", "Restore points and backups with one-click revert", "What changed in each session"],
  },
  settings: {
    stage: 10,
    items: ["Theme, start with Windows, tray behaviour", "Check for updates and open the logs folder"],
  },
};

export function PageView({ page }: { page: PageId }) {
  if (page === "dashboard") return <DashboardPage />;
  if (page === "power") return <PowerPage />;
  if (page === "timer") return <TimerPage />;
  if (page === "debloat") return <DebloatPage />;
  if (page === "tweaks") return <TweaksPage />;
  if (page === "nvidia") return <NvidiaPage />;
  const { stage, items } = planned[page];
  return <PlannedPage route={routeById(page)} stage={stage} items={items} />;
}
