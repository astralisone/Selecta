import { cn } from "@/lib/utils";

/** Mini energy meter — narrow bar with a filled portion and the 0–10 number next to it. */
export function EnergyMeter({ value }: { value: number | null }) {
  if (value == null) {
    return <span className="text-muted-foreground/60 text-xs">—</span>;
  }
  const pct = Math.max(0, Math.min(10, value)) * 10;
  return (
    <div className="flex items-center gap-2 min-w-[72px]">
      <div className="relative h-1.5 w-12 overflow-hidden rounded-full bg-white/5">
        <div
          className={cn("energy-bar absolute inset-y-0 left-0 rounded-full")}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs tabular-nums w-3 text-right font-medium">
        {value}
      </span>
    </div>
  );
}
