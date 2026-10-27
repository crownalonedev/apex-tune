import { useEffect, useState } from "react";
import { animate, motion, useReducedMotion } from "framer-motion";
import type { Score } from "@/lib/tauri";
import { Skeleton } from "@/components/ui/skeleton";

// A 240° gauge, like a tuning dial: from 150° (lower left) clockwise to 30°.
const CX = 100;
const CY = 100;
const R = 80;
const START = 150;
const SWEEP = 240;

function point(angleDeg: number, radius: number) {
  const a = (angleDeg * Math.PI) / 180;
  return { x: CX + radius * Math.cos(a), y: CY + radius * Math.sin(a) };
}

const from = point(START, R);
const to = point(START + SWEEP, R);
const ARC = `M ${from.x} ${from.y} A ${R} ${R} 0 1 1 ${to.x} ${to.y}`;

/** Counts up to `target`, from wherever it currently is. */
function useCountUp(target: number) {
  const reduced = useReducedMotion();
  const [shown, setShown] = useState(reduced ? target : 0);
  useEffect(() => {
    if (reduced) {
      setShown(target);
      return;
    }
    const controls = animate(shown, target, {
      duration: 0.9,
      ease: [0.22, 1, 0.36, 1],
      onUpdate: (v) => setShown(v),
    });
    return () => controls.stop();
    // Re-run only when the target changes; `shown` is the starting point.
  }, [target, reduced]);
  return shown;
}

export function ScoreDial({ score }: { score: Score | null }) {
  const shown = useCountUp(score?.value ?? 0);
  if (!score) {
    return (
      <div className="flex flex-col items-center gap-3 py-4">
        <Skeleton className="size-48 rounded-full" />
        <Skeleton className="h-4 w-40" />
      </div>
    );
  }

  const value = score.value;
  // The needle and arc follow the counting number, so all three move together.
  const needle = point(START + (SWEEP * (value === null ? 0 : shown)) / 100, R - 18);
  const label =
    value === null ? "Optimization score: nothing to score yet" : `Optimization score: ${value} out of 100`;

  return (
    <figure className="flex flex-col items-center">
      <svg viewBox="0 0 200 172" className="w-full max-w-[260px]" role="img" aria-label={label}>
        <path d={ARC} fill="none" stroke="var(--line-strong)" strokeWidth="12" strokeLinecap="round" pathLength={100} />
        {value !== null && shown > 0.5 && (
          <path
            d={ARC}
            fill="none"
            stroke="var(--signal)"
            strokeWidth="12"
            strokeLinecap="round"
            pathLength={100}
            strokeDasharray={`${shown} 100`}
          />
        )}
        {[0, 25, 50, 75, 100].map((tick) => {
          const outer = point(START + (SWEEP * tick) / 100, R + 12);
          const inner = point(START + (SWEEP * tick) / 100, R + 7);
          return (
            <line
              key={tick}
              x1={inner.x}
              y1={inner.y}
              x2={outer.x}
              y2={outer.y}
              stroke="var(--text-faint)"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          );
        })}
        <line
          x1={CX}
          y1={CY}
          x2={needle.x}
          y2={needle.y}
          stroke="var(--text)"
          strokeWidth="4"
          strokeLinecap="round"
        />
        <motion.circle cx={CX} cy={CY} r="7" fill="var(--text)" initial={{ scale: 0 }} animate={{ scale: 1 }} />
        <text
          x={CX}
          y={150}
          textAnchor="middle"
          fill="var(--text)"
          style={{ fontSize: 40, fontWeight: 600, fontStretch: "75%" }}
        >
          {value === null ? "–" : Math.round(shown)}
        </text>
      </svg>
      <figcaption className="-mt-1 text-center">
        <span className="block font-medium">Optimization score</span>
        <span className="block text-[13px] text-muted">
          {score.counted === 0
            ? "Nothing to score yet."
            : `${score.applied} of ${score.counted} recommended tweaks applied`}
        </span>
      </figcaption>
    </figure>
  );
}
