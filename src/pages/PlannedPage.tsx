import { PageHeader } from "@/components/layout/PageHeader";
import type { Route } from "@/lib/routes";

/** Stand-in for pages whose build stage hasn't happened yet. */
export function PlannedPage({ route, stage, items }: { route: Route; stage: number; items: string[] }) {
  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />
      <section className="mt-8 max-w-[68ch]">
        <h2 className="text-[19px] font-semibold">Arrives in build stage {stage}</h2>
        <ul className="mt-3 list-disc space-y-1.5 pl-5 text-muted marker:text-faint">
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </section>
    </>
  );
}
