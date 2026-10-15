import * as React from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { save } from "@tauri-apps/plugin-dialog";
import { Button } from "./ui/Button";
import { Input } from "./ui/Input";
import { exportPlaylist, type ExportFormat, type ExportReport } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Download, FolderOpen } from "lucide-react";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedIds: string[];
  defaultName?: string;
};

type FormatInfo = {
  label: string;
  extension: string;
  filter: { name: string; extensions: string[] };
  howTo: string;
};

// The backend picks the format from the file extension; this table only
// drives the save dialog and the instructions shown.
const FORMATS: Record<ExportFormat, FormatInfo> = {
  m3u8: {
    label: "Rekordbox",
    extension: "m3u8",
    filter: { name: "M3U8 playlist", extensions: ["m3u8", "m3u"] },
    howTo: "Saves an M3U8. In Rekordbox: File → Import Playlist.",
  },
  traktor_nml: {
    label: "Traktor",
    extension: "nml",
    filter: { name: "Traktor playlist", extensions: ["nml"] },
    howTo:
      "Saves an NML playlist carrying BPM and key. In Traktor: right-click Playlists → Import Playlist.",
  },
  serato_crate: {
    label: "Serato",
    extension: "crate",
    filter: { name: "Serato crate", extensions: ["crate"] },
    howTo:
      "Saves a crate. Put it in your _Serato_/Subcrates folder (usually Music/_Serato_/Subcrates) and restart Serato. Only tracks on that folder's drive can be included.",
  },
};

const FORMAT_KEY = "track2mix.exportFormat";

function storedFormat(): ExportFormat {
  const v = localStorage.getItem(FORMAT_KEY);
  return v === "traktor_nml" || v === "serato_crate" || v === "m3u8" ? v : "m3u8";
}

function withExtension(path: string, ext: string): string {
  return /\.[^./\\]+$/.test(path) ? path.replace(/\.[^./\\]+$/, `.${ext}`) : `${path}.${ext}`;
}

function describe(report: ExportReport, path: string): string {
  const parts = [`Wrote ${report.written} track(s) to ${path}.`];
  if (report.skipped_no_path > 0) {
    parts.push(`Skipped ${report.skipped_no_path} with no local file.`);
  }
  if (report.skipped_other_volume > 0) {
    parts.push(
      `Skipped ${report.skipped_other_volume} on a different drive than the crate (e.g. ${report.other_volume_examples[0]}).`,
    );
  }
  return parts.join(" ");
}

export function ExportDialog({ open, onOpenChange, selectedIds, defaultName }: Props) {
  const [name, setName] = React.useState(defaultName ?? "Track2Mix set");
  const [format, setFormat] = React.useState<ExportFormat>(storedFormat);
  const [path, setPath] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [err, setErr] = React.useState<string | null>(null);
  const [ok, setOk] = React.useState<string | null>(null);
  const [warn, setWarn] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setName(defaultName ?? "Track2Mix set");
      setErr(null);
      setOk(null);
    }
  }, [open, defaultName]);

  function chooseFormat(f: ExportFormat) {
    setFormat(f);
    localStorage.setItem(FORMAT_KEY, f);
    if (path) setPath(withExtension(path, FORMATS[f].extension));
    setErr(null);
    setOk(null);
  }

  async function pickPath() {
    const info = FORMATS[format];
    try {
      const picked = await save({
        defaultPath:
          path || `${name.replace(/[^a-z0-9\-_ ]/gi, "")}.${info.extension}`,
        filters: [info.filter],
      });
      if (picked) setPath(picked);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function doExport() {
    if (!path) {
      setErr("Pick a destination file first.");
      return;
    }
    setBusy(true);
    setErr(null);
    setOk(null);
    try {
      const report = await exportPlaylist({
        path,
        playlist_name: name,
        track_ids: selectedIds,
      });
      setOk(describe(report, path));
      setWarn(report.skipped_no_path + report.skipped_other_volume > 0);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-bg-deep/70 backdrop-blur-sm animate-fade-in" />
        <Dialog.Content className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-[480px] glass rounded-xl p-5 animate-fade-in">
          <Dialog.Title className="text-sm font-semibold text-white mb-1">
            Export playlist
          </Dialog.Title>
          <Dialog.Description className="text-xs text-muted-foreground mb-4">
            {selectedIds.length} track{selectedIds.length === 1 ? "" : "s"} selected.{" "}
            {FORMATS[format].howTo}
          </Dialog.Description>

          <div className="space-y-3">
            <div>
              <label className="text-xs text-muted-foreground mb-1 block">
                Export for
              </label>
              <div className="grid grid-cols-3 gap-1 rounded-md bg-white/[0.03] border border-white/10 p-1">
                {(Object.keys(FORMATS) as ExportFormat[]).map((f) => (
                  <button
                    key={f}
                    type="button"
                    onClick={() => chooseFormat(f)}
                    className={cn(
                      "h-7 rounded text-xs transition-colors",
                      f === format
                        ? "bg-iris-500/80 text-white"
                        : "text-muted-foreground hover:text-foreground hover:bg-white/[0.05]",
                    )}
                    aria-pressed={f === format}
                  >
                    {FORMATS[f].label}
                  </button>
                ))}
              </div>
            </div>
            <div>
              <label className="text-xs text-muted-foreground mb-1 block">
                {format === "serato_crate" ? "Crate name (from the file name)" : "Playlist name"}
              </label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <div>
              <label className="text-xs text-muted-foreground mb-1 block">
                Destination
              </label>
              <div className="flex gap-2">
                <Input
                  value={path}
                  readOnly
                  placeholder="Pick a file…"
                  className="flex-1 text-xs"
                />
                <Button variant="outline" size="md" onClick={pickPath}>
                  <FolderOpen className="h-3.5 w-3.5" />
                  Browse
                </Button>
              </div>
            </div>
            {err && (
              <div className="text-xs text-red-300 bg-red-500/10 border border-red-400/20 rounded-md p-2">
                {err}
              </div>
            )}
            {ok && (
              <div
                className={cn(
                  "text-xs rounded-md p-2 border",
                  warn
                    ? "text-gold-300 bg-gold-300/10 border-gold-300/25"
                    : "text-iris-200 bg-iris-500/10 border-iris-500/25",
                )}
              >
                {ok}
              </div>
            )}
          </div>

          <div className="flex justify-end gap-2 mt-5">
            <Dialog.Close asChild>
              <Button variant="ghost">Close</Button>
            </Dialog.Close>
            <Button variant="primary" onClick={doExport} disabled={busy}>
              <Download className="h-3.5 w-3.5" />
              {busy ? "Exporting…" : "Export"}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
