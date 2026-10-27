import { useEffect, useState } from "react";
import { TweakCard } from "@/components/TweakCard";
import { PageHeader } from "@/components/layout/PageHeader";
import { Tabs, TabsContent, TabsList } from "@/components/ui/tabs";
import { routeById } from "@/lib/routes";
import { useTweaksStore } from "@/stores/tweaks";
import { AppsPanel } from "./debloat/AppsPanel";
import { BackgroundPanel } from "./debloat/BackgroundPanel";
import { StartupPanel } from "./debloat/StartupPanel";

const TABS = [
  { value: "apps", label: "Apps" },
  { value: "startup", label: "Startup" },
  { value: "background", label: "Background" },
  { value: "privacy", label: "Suggestions & privacy" },
];

export function DebloatPage() {
  const route = routeById("debloat");
  const [tab, setTab] = useState("apps");
  const loadTweaks = useTweaksStore((s) => s.load);
  const privacy = useTweaksStore((s) => s.overview?.tweaks.filter((t) => t.category === "privacy") ?? []);

  useEffect(() => {
    void loadTweaks();
  }, [loadTweaks]);

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />
      <Tabs value={tab} onValueChange={setTab} className="mt-6">
        <TabsList tabs={TABS} value={tab} />
        <TabsContent value="apps">
          <AppsPanel />
        </TabsContent>
        <TabsContent value="startup">
          <StartupPanel />
        </TabsContent>
        <TabsContent value="background">
          <BackgroundPanel />
        </TabsContent>
        <TabsContent value="privacy">
          <div className="mt-5 grid gap-3">
            {privacy.map((t) => (
              <TweakCard key={t.id} tweak={t} />
            ))}
            {privacy.length === 0 && <p className="text-muted">Loading suggestions and privacy settings…</p>}
          </div>
        </TabsContent>
      </Tabs>
    </>
  );
}
