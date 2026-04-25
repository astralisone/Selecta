import { Search, X } from "lucide-react";
import { Input } from "./ui/Input";
import { Button } from "./ui/Button";
import { cn } from "@/lib/utils";

export type FilterState = {
  search: string;
  genre: string;
  subGenre: string;
  bpmMin: number | "";
  bpmMax: number | "";
  key: string; // substring match on camelot/tonality (e.g. "9A" or "Am")
};

export const emptyFilters: FilterState = {
  search: "",
  genre: "",
  subGenre: "",
  bpmMin: "",
  bpmMax: "",
  key: "",
};

type Props = {
  filters: FilterState;
  onChange: (f: FilterState) => void;
  onReset: () => void;
  hasAny: boolean;
  counts: { shown: number; total: number };
};

export function FilterBar(p: Props) {
  const set = <K extends keyof FilterState>(k: K, v: FilterState[K]) =>
    p.onChange({ ...p.filters, [k]: v });

  return (
    <div
      className={cn(
        "glass rounded-lg p-3 flex flex-wrap items-center gap-2",
      )}
    >
      <div className="relative flex-1 min-w-[240px]">
        <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground/70" />
        <Input
          value={p.filters.search}
          onChange={(e) => set("search", e.target.value)}
          placeholder="Search title or artist"
          className="pl-9"
        />
      </div>
      <Input
        value={p.filters.genre}
        onChange={(e) => set("genre", e.target.value)}
        placeholder="Genre"
        className="w-[120px]"
      />
      <Input
        value={p.filters.subGenre}
        onChange={(e) => set("subGenre", e.target.value)}
        placeholder="Sub-genre"
        className="w-[130px]"
      />
      <Input
        value={p.filters.bpmMin === "" ? "" : String(p.filters.bpmMin)}
        onChange={(e) => {
          const v = e.target.value.trim();
          set("bpmMin", v === "" ? "" : Number(v));
        }}
        placeholder="BPM min"
        className="w-[92px]"
      />
      <Input
        value={p.filters.bpmMax === "" ? "" : String(p.filters.bpmMax)}
        onChange={(e) => {
          const v = e.target.value.trim();
          set("bpmMax", v === "" ? "" : Number(v));
        }}
        placeholder="BPM max"
        className="w-[92px]"
      />
      <Input
        value={p.filters.key}
        onChange={(e) => set("key", e.target.value)}
        placeholder="Key (e.g. 9A)"
        className="w-[110px]"
      />
      {p.hasAny && (
        <Button variant="ghost" size="sm" onClick={p.onReset}>
          <X className="h-3.5 w-3.5" />
          Clear
        </Button>
      )}
      <div className="ml-auto text-xs text-muted-foreground">
        <span className="text-foreground font-medium">
          {p.counts.shown.toLocaleString()}
        </span>{" "}
        / {p.counts.total.toLocaleString()} tracks
      </div>
    </div>
  );
}
