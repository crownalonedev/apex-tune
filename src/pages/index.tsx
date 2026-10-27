import type { PageId } from "@/lib/routes";
import { routeById } from "@/lib/routes";
import { DashboardPage } from "./DashboardPage";
import { PowerPage } from "./PowerPage";
import { TimerPage } from "./TimerPage";
import { PlannedPage } from "./PlannedPage";

const planned: Record<Exclude<PageId, "dashboard" | "power" | "timer">, { stage: number; items: string[] }> = {
  debloat: {
    stage: 6,
    items: [
      "Remove preinstalled apps, with offline restore",
      "Disable startup apps the same way Task Manager does",
      "Turn off suggestions, ads and telemetry tasks; conservative service changes",
    ],
  },
  tweaks: {
    stage: 7,
    items: [
      "Gaming, input, network and visual tweaks with risk and restart badges",
      "Storage cleanup with reclaimed space",
      "Steam library detection and a Minecraft Java RAM helper",
    ],
  },
  nvidia: {
    stage: 8,
    items: [
      "Global low-latency profile and per-game profiles",
      "Full driver settings backup and restore",
    ],
  },
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
  const { stage, items } = planned[page];
  return <PlannedPage route={routeById(page)} stage={stage} items={items} />;
}
