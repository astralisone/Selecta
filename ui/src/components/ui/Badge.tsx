import * as React from "react";
import { cn } from "@/lib/utils";

type Variant = "iris" | "gold" | "muted" | "outline";

export function Badge({
  variant = "muted",
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & { variant?: Variant }) {
  const variants: Record<Variant, string> = {
    iris: "pill-iris",
    gold: "pill-gold",
    muted: "pill-muted",
    outline: "border border-white/10 text-muted-foreground",
  };
  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium tracking-wide",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}
