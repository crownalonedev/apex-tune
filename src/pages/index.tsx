import type { PageId } from "@/lib/routes";
import { DashboardPage } from "./DashboardPage";
import { DebloatPage } from "./DebloatPage";
import { NvidiaPage } from "./NvidiaPage";
import { PowerPage } from "./PowerPage";
import { ProfilesPage } from "./ProfilesPage";
import { SettingsPage } from "./SettingsPage";
import { TimerPage } from "./TimerPage";
import { TweaksPage } from "./TweaksPage";

const PAGES: Record<PageId, () => React.JSX.Element> = {
  dashboard: DashboardPage,
  debloat: DebloatPage,
  power: PowerPage,
  timer: TimerPage,
  nvidia: NvidiaPage,
  tweaks: TweaksPage,
  profiles: ProfilesPage,
  settings: SettingsPage,
};

export function PageView({ page }: { page: PageId }) {
  const Page = PAGES[page];
  return <Page />;
}
