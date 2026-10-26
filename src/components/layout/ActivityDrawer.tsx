import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useActivityStore } from "@/stores/activity";
import type { ActivityEntry } from "@/lib/tauri";

const timeFormat = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
});

/** Docked panel along the bottom of the content area. */
export function ActivityDrawer() {
  const open = useActivityStore((s) => s.open);
  const setOpen = useActivityStore((s) => s.setOpen);
  const entries = useActivityStore((s) => s.entries);
  const listRef = useRef<HTMLOListElement>(null);

  // Keep the newest entry in view.
  useEffect(() => {
    const list = listRef.current;
    if (open && list) list.scrollTop = list.scrollHeight;
  }, [open, entries.length]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, setOpen]);

  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.section
          id="activity-log"
          aria-label="Activity log"
          initial={{ height: 0 }}
          animate={{ height: 260 }}
          exit={{ height: 0 }}
          transition={{ duration: 0.18, ease: "easeOut" }}
          className="shrink-0 overflow-hidden border-t border-line bg-panel"
        >
          <div className="flex h-10 items-center justify-between border-b border-line px-5">
            <h2 className="text-[15px] font-semibold">Activity log</h2>
            <Button variant="ghost" size="icon" onClick={() => setOpen(false)} aria-label="Close activity log">
              <X />
            </Button>
          </div>
          {entries.length === 0 ? (
            <p className="px-5 py-4 text-muted">
              Every change Apex Tune makes, and every error, is listed here and saved to the log file.
            </p>
          ) : (
            <ol ref={listRef} className="selectable h-[218px] overflow-y-auto px-5 py-2 text-[13px]">
              {entries.map((entry) => (
                <ActivityRow key={entry.id} entry={entry} />
              ))}
            </ol>
          )}
        </motion.section>
      )}
    </AnimatePresence>
  );
}

function ActivityRow({ entry }: { entry: ActivityEntry }) {
  return (
    <li className="flex gap-4 py-1">
      <time dateTime={entry.at} className="w-[72px] shrink-0 text-faint">
        {timeFormat.format(new Date(entry.at))}
      </time>
      <span
        className={cn(
          "min-w-0 flex-1",
          entry.level === "error" && "text-danger",
          entry.level === "warn" && "text-moderate",
        )}
      >
        {entry.level !== "info" && <span className="sr-only">{entry.level}: </span>}
        {entry.message}
      </span>
      {entry.subject && <span className="shrink-0 text-faint">{entry.subject}</span>}
    </li>
  );
}
