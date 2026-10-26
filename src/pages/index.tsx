import type { PageId } from "@/lib/routes";
import { routeById } from "@/lib/routes";
import { DashboardPage } from "./DashboardPage";
import { PlannedPage } from "./PlannedPage";

const planned: Record<Exclude<PageId, "dashboard">, { stage: number; items: string[] }> = {
  power: {
    stage: 4,
    items: [
      "Install the Apex Tune Gaming plan, duplicated from Ultimate Performance",
      "Laptop-safe variant when a battery is detected",
      "Switch plans or restore Balanced",
    ],
  },
  timer: {
    stage: 5,
    items: [
      "Current, minimum and maximum timer resolution",
      "Sleep(1) accuracy benchmark with a recommended value",
      "Tray helper that holds the resolution while active",
    ],
  },
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
  const { stage, items } = planned[page];
  return <PlannedPage route={routeById(page)} stage={stage} items={items} />;
}
