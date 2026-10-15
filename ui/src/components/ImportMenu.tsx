import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { ChevronDown, Disc3, FileUp, FolderOpen, FolderSearch, ListMusic } from "lucide-react";
import type { SourceKind } from "@/lib/tauri";
import { Button, type ButtonProps } from "./ui/Button";

export type ImportChoice = { kind: SourceKind; path: string };

type Option = {
  id: string;
  kind: SourceKind;
  label: string;
  hint: string;
  icon: React.ComponentType<{ className?: string }>;
  pick: () => Promise<string | null>;
};

async function pickOne(opts: Parameters<typeof openFile>[0]): Promise<string | null> {
  const picked = await openFile({ ...opts, multiple: false });
  return typeof picked === "string" ? picked : null;
}

export const IMPORT_OPTIONS: Option[] = [
  {
    id: "rekordbox",
    kind: "rekordbox",
    label: "Rekordbox XML",
    hint: "File → Export Collection in xml format",
    icon: FileUp,
    pick: () => pickOne({ filters: [{ name: "Rekordbox XML", extensions: ["xml"] }] }),
  },
  {
    id: "traktor",
    kind: "traktor",
    label: "Traktor collection",
    hint: "collection.nml, or an exported .nml playlist",
    icon: Disc3,
    pick: () => pickOne({ filters: [{ name: "Traktor NML", extensions: ["nml"] }] }),
  },
  {
    id: "serato",
    kind: "serato",
    label: "Serato library",
    hint: "Pick your _Serato_ folder (usually Music/_Serato_)",
    icon: FolderOpen,
    pick: () => pickOne({ directory: true, title: "Select your _Serato_ folder" }),
  },
  {
    id: "serato-crate",
    kind: "serato",
    label: "Serato crate",
    hint: "One crate from _Serato_/Subcrates",
    icon: ListMusic,
    pick: () => pickOne({ filters: [{ name: "Serato crate", extensions: ["crate"] }] }),
  },
  {
    id: "directory",
    kind: "directory",
    label: "Scan a folder",
    hint: "Every audio file underneath, read from its tags",
    icon: FolderSearch,
    pick: () => pickOne({ directory: true, title: "Select a music folder to scan" }),
  },
];

type Props = {
  onPick: (choice: ImportChoice) => void;
  onError: (message: string) => void;
  disabled?: boolean;
  variant?: ButtonProps["variant"];
  size?: ButtonProps["size"];
};

/** "Import ▾" button listing every library source Track2Mix can read. */
export function ImportMenu({ onPick, onError, disabled, variant = "ghost", size = "sm" }: Props) {
  async function choose(opt: Option) {
    try {
      const path = await opt.pick();
      if (path) onPick({ kind: opt.kind, path });
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild disabled={disabled}>
        <Button variant={variant} size={size} disabled={disabled}>
          <FileUp className="h-3.5 w-3.5" />
          Import / scan
          <ChevronDown className="h-3 w-3 opacity-60" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          sideOffset={6}
          className="z-50 w-72 glass rounded-lg border border-white/10 p-1 animate-fade-in"
        >
          {IMPORT_OPTIONS.map((opt) => (
            <DropdownMenu.Item
              key={opt.id}
              onSelect={() => void choose(opt)}
              className="flex items-start gap-2.5 rounded-md px-2.5 py-2 text-left outline-none cursor-pointer data-[highlighted]:bg-iris-500/15"
            >
              <opt.icon className="h-4 w-4 mt-0.5 text-iris-300 shrink-0" />
              <span className="min-w-0">
                <span className="block text-xs text-foreground">{opt.label}</span>
                <span className="block text-[11px] text-muted-foreground">{opt.hint}</span>
              </span>
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
