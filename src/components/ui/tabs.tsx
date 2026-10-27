import { useId } from "react";
import { Tabs as Primitive } from "radix-ui";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";

export const Tabs = Primitive.Root;
export const TabsContent = Primitive.Content;

/** A tab row whose active marker slides between tabs. */
export function TabsList({
  tabs,
  value,
  className,
}: {
  tabs: { value: string; label: string; count?: number }[];
  value: string;
  className?: string;
}) {
  const marker = useId();
  return (
    <Primitive.List className={cn("flex gap-1 border-b border-line", className)}>
      {tabs.map((t) => (
        <Primitive.Trigger
          key={t.value}
          value={t.value}
          className={cn(
            "relative -mb-px px-3 pt-2 pb-2.5 text-sm transition-colors",
            value === t.value ? "text-text" : "text-muted hover:text-text",
          )}
        >
          {t.label}
          {t.count !== undefined && <span className="ml-1.5 text-[12px] text-faint tabular-nums">{t.count}</span>}
          {value === t.value && (
            <motion.span
              layoutId={marker}
              className="absolute inset-x-2 bottom-0 h-0.5 rounded-full bg-signal"
              transition={{ type: "spring", stiffness: 500, damping: 40 }}
            />
          )}
        </Primitive.Trigger>
      ))}
    </Primitive.List>
  );
}
