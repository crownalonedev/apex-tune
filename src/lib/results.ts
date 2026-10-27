import { toast } from "sonner";
import { plural } from "@/lib/format";
import type { ItemResult } from "@/lib/tauri";

/** One toast for a batch: success count, plus each failure and why. */
export function reportResults(results: ItemResult[], verb: string, noun: string) {
  const ok = results.filter((r) => r.ok).length;
  const failed = results.filter((r) => !r.ok);
  if (failed.length === 0) {
    toast.success(`${verb} ${plural(ok, noun)}`);
    return;
  }
  toast.warning(`${verb} ${ok} of ${results.length}. ${plural(failed.length, noun)} couldn't be changed and ${failed.length === 1 ? "was" : "were"} left as ${failed.length === 1 ? "it was" : "they were"}.`, {
    description: failed.map((f) => `${f.id}: ${f.error}`).join("\n"),
    duration: 15_000,
  });
}
