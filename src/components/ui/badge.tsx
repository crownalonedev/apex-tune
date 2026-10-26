import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-sm border px-1.5 py-px text-[12px] font-medium leading-5",
  {
    variants: {
      tone: {
        neutral: "border-line text-muted",
        signal: "border-signal/40 bg-signal-soft text-signal",
        safe: "border-safe/40 text-safe",
        moderate: "border-moderate/40 text-moderate",
        advanced: "border-advanced/40 text-advanced",
      },
    },
    defaultVariants: { tone: "neutral" },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, tone, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ tone }), className)} {...props} />;
}
