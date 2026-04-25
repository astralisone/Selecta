import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatBpm(bpm: number | null | undefined): string {
  if (bpm == null || bpm <= 0) return "—";
  return bpm.toFixed(1);
}
