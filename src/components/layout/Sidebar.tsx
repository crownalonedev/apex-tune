import { ShieldAlert, ShieldCheck, ScrollText } from "lucide-react";
import { routes } from "@/lib/routes";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app";
import { useActivityStore } from "@/stores/activity";
import { inTauri } from "@/lib/tauri";

export function Sidebar() {
  const page = useAppStore((s) => s.page);
  const setPage = useAppStore((s) => s.setPage);

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-panel">
      <div className="flex items-center gap-2.5 px-5 pt-5 pb-6">
        <BrandMark />
        <span className="text-[19px] font-semibold [font-stretch:78%]">Apex Tune</span>
      </div>

      <nav aria-label="Main" className="flex-1 overflow-y-auto px-3">
        <ul className="space-y-0.5">
          {routes.map((route) => {
            const active = route.id === page;
            const Icon = route.icon;
            return (
              <li key={route.id}>
                <button
                  type="button"
                  onClick={() => setPage(route.id)}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "relative flex h-9 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors",
                    active
                      ? "bg-panel-raised text-text"
                      : "text-muted hover:bg-panel-raised/60 hover:text-text",
                  )}
                >
                  {active && (
                    <span
                      aria-hidden="true"
                      className="absolute top-2 bottom-2 left-0 w-0.5 rounded-full bg-signal"
                    />
                  )}
                  <Icon className={cn("size-4", active && "text-signal")} aria-hidden="true" />
                  {route.label}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      <div className="space-y-1 border-t border-line p-3">
        <ElevationStatus />
        <ActivityButton />
      </div>
    </aside>
  );
}

function ElevationStatus() {
  const info = useAppStore((s) => s.info);
  if (!info) return null;

  if (!inTauri()) {
    return (
      <p className="px-3 py-2 text-[12px] leading-snug text-faint">
        Browser preview. Nothing here touches your system.
      </p>
    );
  }

  return info.elevated ? (
    <p className="flex items-center gap-2 px-3 py-2 text-[12px] text-muted">
      <ShieldCheck className="size-4 text-safe" aria-hidden="true" />
      Running as administrator
    </p>
  ) : (
    <p className="flex items-start gap-2 px-3 py-2 text-[12px] leading-snug text-moderate">
      <ShieldAlert className="mt-px size-4 shrink-0" aria-hidden="true" />
      Not running as administrator. Changes will fail until you restart Apex Tune elevated.
    </p>
  );
}

function ActivityButton() {
  const open = useActivityStore((s) => s.open);
  const setOpen = useActivityStore((s) => s.setOpen);
  const unseen = useActivityStore((s) => s.unseenProblems);

  return (
    <button
      type="button"
      onClick={() => setOpen(!open)}
      aria-expanded={open}
      aria-controls="activity-log"
      className="flex h-9 w-full items-center gap-3 rounded-md px-3 text-sm text-muted transition-colors hover:bg-panel-raised hover:text-text"
    >
      <ScrollText className="size-4" aria-hidden="true" />
      Activity log
      {unseen > 0 && (
        <span className="ml-auto rounded-sm bg-danger/15 px-1.5 text-[12px] font-medium text-danger">
          {unseen}
          <span className="sr-only"> new problems</span>
        </span>
      )}
    </button>
  );
}

function BrandMark() {
  return (
    <svg viewBox="0 0 1024 1024" className="size-7" aria-hidden="true">
      <rect x="64" y="64" width="896" height="896" rx="200" fill="var(--panel-raised)" stroke="var(--line-strong)" strokeWidth="24" />
      <path d="M232 660 A300 300 0 1 1 792 660" fill="none" stroke="var(--line-strong)" strokeWidth="64" strokeLinecap="round" />
      <path d="M232 660 A300 300 0 0 1 672 300" fill="none" stroke="var(--signal)" strokeWidth="64" strokeLinecap="round" />
      <line x1="512" y1="600" x2="676" y2="372" stroke="var(--text)" strokeWidth="44" strokeLinecap="round" />
      <circle cx="512" cy="600" r="56" fill="var(--text)" />
    </svg>
  );
}
