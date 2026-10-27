import { useEffect, useRef } from "react";
import { AnimatePresence, MotionConfig, motion } from "framer-motion";
import { Toaster, toast } from "sonner";
import { api } from "@/lib/tauri";
import { checkForUpdate } from "@/lib/updates";
import { ActivityDrawer } from "@/components/layout/ActivityDrawer";
import { RestartBanner } from "@/components/layout/RestartBanner";
import { Sidebar } from "@/components/layout/Sidebar";
import { PageView } from "@/pages";
import { useActivityStore } from "@/stores/activity";
import { useAppStore } from "@/stores/app";
import { motionSetting, resolveTheme, useSettingsStore, watchSystemPreferences } from "@/stores/settings";

export default function App() {
  const page = useAppStore((s) => s.page);
  const loadInfo = useAppStore((s) => s.loadInfo);
  const connectActivity = useActivityStore((s) => s.connect);
  const loadSettings = useSettingsStore((s) => s.load);
  const settings = useSettingsStore((s) => s.view);
  const checkedUpdates = useRef(false);
  const mainRef = useRef<HTMLElement>(null);

  useEffect(() => {
    void loadInfo();
    void loadSettings();
    const unwatch = watchSystemPreferences();
    // Changes interrupted last time were rolled back at startup; say so once.
    void api.takeRecoveryMessages().then((messages) => {
      for (const message of messages) toast.warning(message, { duration: 15_000 });
    });
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void connectActivity().then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
      unwatch();
    };
  }, [loadInfo, loadSettings, connectActivity]);

  // One quiet update check per launch, only if allowed and configured.
  useEffect(() => {
    if (!settings?.checkForUpdates || checkedUpdates.current) return;
    checkedUpdates.current = true;
    void checkForUpdate(settings.version).then((r) => {
      if (r.kind === "available") toast.info(`Apex Tune ${r.latest} is available`, { description: r.url, duration: 15_000 });
    });
  }, [settings]);

  // Each page starts at the top.
  useEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = 0;
  }, [page]);

  const theme = settings ? resolveTheme(settings.theme) : "graphite";

  return (
    <MotionConfig reducedMotion={motionSetting(settings?.motion)}>
      <div className="flex h-full">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <RestartBanner />
          <main ref={mainRef} className="flex-1 overflow-y-auto px-10 pt-8 pb-12">
            <div className="mx-auto max-w-[1180px]">
              <AnimatePresence mode="wait" initial={false}>
                <motion.div
                  key={page}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={{ duration: 0.16, ease: "easeOut" }}
                >
                  <PageView page={page} />
                </motion.div>
              </AnimatePresence>
            </div>
          </main>
          <ActivityDrawer />
        </div>
        <Toaster
          theme={theme === "daylight" ? "light" : "dark"}
          position="bottom-right"
          toastOptions={{
            className: "!bg-panel-raised !border-line-strong !text-text !font-sans",
          }}
        />
      </div>
    </MotionConfig>
  );
}
