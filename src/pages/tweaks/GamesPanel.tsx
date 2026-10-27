import { useEffect, useState } from "react";
import { toast } from "sonner";
import { RiskBadge } from "@/components/badges";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { api, toAppError, type Collector, type Game, type JvmAdvice } from "@/lib/tauri";
import { cn } from "@/lib/utils";

type Pending = { game: Game; kind: "priority" | "fso" } | null;

export function GamesPanel() {
  const [games, setGames] = useState<Game[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<Pending>(null);

  const load = () =>
    api
      .gamesList()
      .then(setGames)
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void load();
  }, []);

  const change = async (game: Game, kind: "priority" | "fso", on: boolean) => {
    setBusy(true);
    try {
      if (kind === "priority") await api.gameSetPriority(game.exe, on);
      else await api.gameSetFso(game.path!, on);
      toast.success(`${game.name}: ${kind === "priority" ? "CPU priority" : "fullscreen optimizations"} ${on ? "changed" : "restored"}`);
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(false);
      await load();
    }
  };

  return (
    <div className="mt-5 space-y-6">
      <p className="max-w-[72ch] text-muted">
        Settings Windows applies when a game starts. Apex Tune never touches a running game, so anti-cheat has
        nothing to see. Found on this PC: known titles, Steam and Epic libraries.
      </p>
      {!games ? (
        <Skeleton className="h-40 w-full" />
      ) : (
        <ul className="divide-y divide-line border-y border-line">
          {games.map((g) => (
            <li key={g.exe + (g.path ?? "")} className="flex flex-wrap items-start gap-x-6 gap-y-3 py-3">
              <div className="min-w-0 flex-1 basis-72">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-medium">{g.name}</span>
                  <Badge>{g.source === "known" ? "Detected" : g.source === "steam" ? "Steam" : "Epic"}</Badge>
                </div>
                <p className="selectable truncate text-[12px] text-faint" title={g.path ?? g.exe}>
                  {g.path ?? g.exe}
                </p>
                {g.note && <p className="text-[12px] text-muted">{g.note}</p>}
              </div>
              <label className="flex items-center gap-2 text-[13px]">
                <Switch
                  checked={g.highPriority}
                  disabled={busy}
                  onCheckedChange={(v) => (v ? setPending({ game: g, kind: "priority" }) : void change(g, "priority", false))}
                  aria-label={`High CPU priority for ${g.name}`}
                />
                High CPU priority
              </label>
              <label className={cn("flex items-center gap-2 text-[13px]", !g.path && "opacity-50")}>
                <Switch
                  checked={g.fsoDisabled}
                  disabled={busy || !g.path}
                  onCheckedChange={(v) => (v ? setPending({ game: g, kind: "fso" }) : void change(g, "fso", false))}
                  aria-label={`Fullscreen optimizations off for ${g.name}`}
                />
                Fullscreen optimizations off
              </label>
            </li>
          ))}
        </ul>
      )}

      <AlertDialog open={pending !== null} onOpenChange={(o) => !o && setPending(null)}>
        <AlertDialogContent>
          {pending && (
            <>
              <AlertDialogTitle>
                {pending.kind === "priority" ? "High CPU priority" : "Fullscreen optimizations off"} for {pending.game.name}?
              </AlertDialogTitle>
              <div className="mt-2">
                <RiskBadge risk={pending.kind === "priority" ? "moderate" : "advanced"} />
              </div>
              <AlertDialogDescription>
                {pending.kind === "priority"
                  ? "Windows starts the game at High priority (via Image File Execution Options), so background apps get CPU time after it. Helps 1% lows on busy CPUs; can make other apps sluggish while you play. Takes effect next launch."
                  : "Turns off Windows' fullscreen optimizations for this game only. On Windows 11 these usually help, so only do this if this game stutters or misbehaves in exclusive fullscreen."}
              </AlertDialogDescription>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction variant="primary" onClick={() => void change(pending.game, pending.kind, true)}>
                  Turn on
                </AlertDialogAction>
              </AlertDialogFooter>
            </>
          )}
        </AlertDialogContent>
      </AlertDialog>

      <MinecraftCard />
    </div>
  );
}

function MinecraftCard() {
  const [modded, setModded] = useState(false);
  const [collector, setCollector] = useState<Collector>("g1");
  const [advice, setAdvice] = useState<JvmAdvice | null>(null);

  useEffect(() => {
    void api.minecraftJvm(modded, collector).then(setAdvice);
  }, [modded, collector]);

  const copy = async () => {
    if (!advice) return;
    try {
      await navigator.clipboard.writeText(advice.arguments);
      toast.success("JVM arguments copied");
    } catch {
      toast.error("Couldn't copy. Select the text and copy it instead.");
    }
  };

  const option = (active: boolean) =>
    cn("h-8 rounded-md border px-3 text-sm", active ? "border-signal bg-signal-soft text-signal" : "border-line-strong text-muted");

  return (
    <section className="rounded-lg border border-line bg-panel p-4">
      <h3 className="text-[17px] font-semibold">Minecraft Java memory and JVM arguments</h3>
      <p className="mt-1 max-w-[72ch] text-[13px] text-muted">Sized to this PC's memory. Apex Tune doesn't edit launcher files; copy these in yourself.</p>
      <div className="mt-3 flex flex-wrap gap-2">
        <button type="button" aria-pressed={!modded} className={option(!modded)} onClick={() => setModded(false)}>
          Vanilla or light mods
        </button>
        <button type="button" aria-pressed={modded} className={option(modded)} onClick={() => setModded(true)}>
          Modpack
        </button>
        <span className="mx-1 w-px self-stretch bg-line" aria-hidden="true" />
        <button type="button" aria-pressed={collector === "g1"} className={option(collector === "g1")} onClick={() => setCollector("g1")}>
          G1 (recommended)
        </button>
        <button type="button" aria-pressed={collector === "zgc"} className={option(collector === "zgc")} onClick={() => setCollector("zgc")}>
          ZGC (Java 21+)
        </button>
      </div>
      {advice && (
        <>
          <p className="mt-3 text-[13px]">
            Allocate <strong>{advice.memoryGb} GB</strong>
          </p>
          <pre className="selectable mt-2 overflow-x-auto rounded-md border border-line bg-bg p-3 text-[12px] leading-relaxed whitespace-pre-wrap break-all">
            {advice.arguments}
          </pre>
          <div className="mt-2 flex justify-end">
            <Button size="sm" onClick={() => void copy()}>
              Copy arguments
            </Button>
          </div>
          <ul className="mt-2 list-disc space-y-1 pl-5 text-[12px] text-muted">
            {advice.notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
