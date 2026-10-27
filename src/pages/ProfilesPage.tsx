import { useState } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Tabs, TabsContent, TabsList } from "@/components/ui/tabs";
import { routeById } from "@/lib/routes";
import { HistoryPanel } from "./profiles/HistoryPanel";
import { ProfilesPanel } from "./profiles/ProfilesPanel";
import { RestorePointsPanel } from "./profiles/RestorePointsPanel";

const TABS = [
  { value: "profiles", label: "Profiles" },
  { value: "history", label: "What changed" },
  { value: "restore", label: "Restore points" },
];

export function ProfilesPage() {
  const route = routeById("profiles");
  const [tab, setTab] = useState("profiles");
  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />
      <Tabs value={tab} onValueChange={setTab} className="mt-6">
        <TabsList tabs={TABS} value={tab} />
        <TabsContent value="profiles">
          <ProfilesPanel />
        </TabsContent>
        <TabsContent value="history">
          <HistoryPanel />
        </TabsContent>
        <TabsContent value="restore">
          <RestorePointsPanel />
        </TabsContent>
      </Tabs>
    </>
  );
}
