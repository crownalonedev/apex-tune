import { Dialog as Primitive } from "radix-ui";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

export const Dialog = Primitive.Root;
export const DialogTrigger = Primitive.Trigger;
export const DialogClose = Primitive.Close;

export function DialogContent({ className, children, ...props }: Primitive.DialogContentProps) {
  return (
    <Primitive.Portal>
      <Primitive.Overlay className="fixed inset-0 z-50 bg-black/55" />
      <Primitive.Content
        className={cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[min(88vh,760px)] w-[min(94vw,44rem)] -translate-x-1/2 -translate-y-1/2 flex-col rounded-lg border border-line-strong bg-panel shadow-2xl shadow-black/40",
          className,
        )}
        {...props}
      >
        {children}
        <Primitive.Close
          className="absolute top-4 right-4 rounded-md p-1 text-muted hover:bg-panel-raised hover:text-text"
          aria-label="Close"
        >
          <X className="size-4" />
        </Primitive.Close>
      </Primitive.Content>
    </Primitive.Portal>
  );
}

export function DialogTitle({ className, ...props }: Primitive.DialogTitleProps) {
  return <Primitive.Title className={cn("text-[23px] font-semibold [font-stretch:78%]", className)} {...props} />;
}

export function DialogDescription({ className, ...props }: Primitive.DialogDescriptionProps) {
  return <Primitive.Description className={cn("mt-1 text-muted", className)} {...props} />;
}
