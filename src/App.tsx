import { useEffect } from "react";
import { Toaster } from "sonner";
import { ActivityDrawer } from "@/components/layout/ActivityDrawer";
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
