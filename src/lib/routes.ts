import {
  Gauge,
  PackageMinus,
  Zap,
  Timer,
  Cpu,
  SlidersHorizontal,
  History,
  Settings,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "debloat"
  | "power"
  | "timer"
  | "nvidia"
  | "tweaks"
  | "profiles"
  | "settings";

export interface Route {
  id: PageId;
  label: string;
  /** One line shown under the page title. */
  summary: string;
  icon: LucideIcon;
}

export const routes: readonly Route[] = [
  {
    id: "dashboard",
    label: "Dashboard",
    summary: "Your hardware, live load, and how much of the gaming preset is applied.",
    icon: Gauge,
  },
  {
    id: "debloat",
    label: "Debloat",
    summary: "Remove preinstalled apps, quiet startup items, and turn off suggestions and ads.",
    icon: PackageMinus,
  },
  {
    id: "power",
    label: "Power",
    summary: "Install the Apex Tune Gaming plan or switch between your power plans.",
    icon: Zap,
  },
  {
    id: "timer",
    label: "Timer",
    summary: "Measure and set the system timer resolution.",
    icon: Timer,
  },
  {
    id: "nvidia",
    label: "NVIDIA",
    summary: "Driver profile settings for low latency, globally and per game.",
    icon: Cpu,
  },
  {
    id: "tweaks",
    label: "Tweaks",
    summary: "Individual gaming, input, network, visual and storage settings.",
    icon: SlidersHorizontal,
  },
  {
    id: "profiles",
    label: "Profiles & backups",
    summary: "Saved tweak profiles, restore points, and everything you can revert.",
    icon: History,
  },
  {
    id: "settings",
    label: "Settings",
    summary: "Theme, startup and tray behaviour, updates, and logs.",
    icon: Settings,
  },
] as const;

export function routeById(id: PageId): Route {
  const route = routes.find((r) => r.id === id);
  if (!route) throw new Error(`Unknown page: ${id}`);
  return route;
}
