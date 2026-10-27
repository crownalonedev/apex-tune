import * as React from "react";
import { Switch as Primitive } from "radix-ui";
import { cn } from "@/lib/utils";

export const Switch = React.forwardRef<HTMLButtonElement, Primitive.SwitchProps>(({ className, ...props }, ref) => (
  <Primitive.Root
    ref={ref}
    className={cn(
      "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border border-line-strong bg-panel-raised transition-colors",
      "data-[state=checked]:border-signal data-[state=checked]:bg-signal disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  >
    <Primitive.Thumb
      className={cn(
        "pointer-events-none block size-3.5 translate-x-0.5 rounded-full bg-muted shadow transition-transform duration-200 ease-[cubic-bezier(.3,1.5,.6,1)]",
        "data-[state=checked]:translate-x-[18px] data-[state=checked]:bg-signal-ink",
      )}
    />
  </Primitive.Root>
));
Switch.displayName = "Switch";
