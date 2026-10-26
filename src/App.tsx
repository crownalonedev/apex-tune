import { useEffect } from "react";
import { Toaster, toast } from "sonner";
import { api } from "@/lib/tauri";
import { ActivityDrawer } from "@/components/layout/ActivityDrawer";
import { RestartBanner } from "@/components/layout/RestartBanner";
import { Sidebar } from "@/components/layout/Sidebar";
import { PageView } from "@/pages";
import { useActivityStore } from "@/stores/activity";
import { useAppStore } from "@/stores/app";

export default function App() {
  const page = useAppStore((s) => s.page);
  const loadInfo = useAppStore((s) => s.loadInfo);
  const connectActivity = useActivityStore((s) => s.connect);

  useEffect(() => {
    void loadInfo();
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
    };
  }, [loadInfo, connectActivity]);

  return (
    <div className="flex h-full">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <RestartBanner />
        <main className="flex-1 overflow-y-auto px-10 pt-8 pb-12">
          <div className="mx-auto max-w-[1180px]">
            <PageView page={page} />
          </div>
        </main>
        <ActivityDrawer />
      </div>
      <Toaster
        theme="dark"
        position="bottom-right"
        toastOptions={{
          className: "!bg-panel-raised !border-line-strong !text-text !font-sans",
        }}
      />
    </div>
  );
}
