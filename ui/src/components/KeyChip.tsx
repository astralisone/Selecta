import { cn } from "@/lib/utils";

/** Camelot key chip. Outer ring (B/major) is warmer; inner ring (A/minor) is cooler. */
export function KeyChip({ camelot, raw }: { camelot: string | null; raw: string }) {
  if (!camelot) {
    if (!raw.trim()) {
      return <span className="text-muted-foreground/60 text-xs">—</span>;
    }
    return (
      <span className="text-xs text-muted-foreground/80 font-mono">{raw}</span>
    );
  }
  const isMajor = camelot.endsWith("B");
  return (
    <span
      className={cn(
        "inline-flex items-center justify-center rounded-md font-mono text-[11px] font-medium px-1.5 py-0.5 min-w-[32px] tabular-nums",
        isMajor
          ? "bg-gold-300/15 text-gold-200 border border-gold-300/25"
          : "bg-iris-500/15 text-iris-200 border border-iris-500/30",
      )}
    >
      {camelot}
    </span>
  );
}
