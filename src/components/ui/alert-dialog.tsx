import * as React from "react";
import { AlertDialog as Primitive } from "radix-ui";
import { cn } from "@/lib/utils";
import { Button, type ButtonProps } from "./button";

export const AlertDialog = Primitive.Root;
export const AlertDialogTrigger = Primitive.Trigger;

export function AlertDialogContent({ className, children, ...props }: Primitive.AlertDialogContentProps) {
  return (
    <Primitive.Portal>
      <Primitive.Overlay className="fixed inset-0 z-50 bg-black/55" />
      <Primitive.Content
        className={cn(
          "fixed top-1/2 left-1/2 z-50 w-[min(92vw,30rem)] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-line-strong bg-panel p-6 shadow-2xl shadow-black/40",
          className,
        )}
        {...props}
      >
        {children}
      </Primitive.Content>
    </Primitive.Portal>
  );
}

export function AlertDialogTitle({ className, ...props }: Primitive.AlertDialogTitleProps) {
  return <Primitive.Title className={cn("text-[21px] font-semibold [font-stretch:78%]", className)} {...props} />;
}

export function AlertDialogDescription({ className, ...props }: Primitive.AlertDialogDescriptionProps) {
  return <Primitive.Description className={cn("mt-2 text-muted", className)} {...props} />;
}

export function AlertDialogFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mt-6 flex justify-end gap-2", className)} {...props} />;
}

export function AlertDialogCancel(props: ButtonProps) {
  return (
    <Primitive.Cancel asChild>
      <Button variant="ghost" {...props} />
    </Primitive.Cancel>
  );
}

export function AlertDialogAction(props: ButtonProps) {
  return (
    <Primitive.Action asChild>
      <Button {...props} />
    </Primitive.Action>
  );
}
