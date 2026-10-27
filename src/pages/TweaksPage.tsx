import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { TweakCard } from "@/components/TweakCard";
import { PageHeader } from "@/components/layout/PageHeader";
import { Tabs, TabsContent, TabsList } from "@/components/ui/tabs";
import { routeById } from "@/lib/routes";
import type { Category, TweakInfo } from "@/lib/tauri";
import { useTweaksStore } from "@/stores/tweaks";
import { DnsCard } from "./tweaks/DnsCard";
import { GamesPanel } from "./tweaks/GamesPanel";
import { PauseUpdatesCard } from "./tweaks/PauseUpdatesCard";
import { StoragePanel } from "./tweaks/StoragePanel";

const TABS = [
  { value: "gaming", label: "Gaming" },
  { value: "input", label: "Input & visual" },
  { value: "network", label: "Network" },
  { value: "updates", label: "Updates & system" },
  { value: "games", label: "Games" },
  { value: "storage", label: "Storage" },
];

function CardList({ tweaks }: { tweaks: TweakInfo[] }) {
  return (
    <div className="grid gap-3">
      {tweaks.map((t, i) => (
        <motion.div
          key={t.id}
          initial={{ opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: i * 0.03, duration: 0.2 }}
        >
          <TweakCard tweak={t} />
        </motion.div>
      ))}
    </div>
  );
}

export function TweaksPage() {
  const route = routeById("tweaks");
  const [tab, setTab] = useState("gaming");
  const load = useTweaksStore((s) => s.load);
  const tweaks = useTweaksStore((s) => s.overview?.tweaks ?? []);

  useEffect(() => {
    void load();
  }, [load]);

  const of = (...cats: Category[]) => tweaks.filter((t) => cats.includes(t.category));

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />
      <Tabs value={tab} onValueChange={setTab} className="mt-6">
        <TabsList tabs={TABS} value={tab} />
        <TabsContent value="gaming" className="mt-5">
          <CardList tweaks={of("gaming")} />
        </TabsContent>
        <TabsContent value="input" className="mt-5 space-y-3">
          <CardList tweaks={of("input", "visual")} />
        </TabsContent>
        <TabsContent value="network" className="mt-5 space-y-3">
          <DnsCard />
          <CardList tweaks={of("network")} />
        </TabsContent>
        <TabsContent value="updates" className="mt-5 space-y-3">
          <PauseUpdatesCard />
          <CardList tweaks={of("updates", "system")} />
        </TabsContent>
        <TabsContent value="games">
          <GamesPanel />
        </TabsContent>
        <TabsContent value="storage">
          <StoragePanel />
        </TabsContent>
      </Tabs>
    </>
  );
}
