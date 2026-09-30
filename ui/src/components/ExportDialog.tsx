import * as React from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { save } from "@tauri-apps/plugin-dialog";
import { Button } from "./ui/Button";
import { Input } from "./ui/Input";
import { exportPlaylist } from "@/lib/tauri";
import { Download, FolderOpen } from "lucide-react";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedIds: string[];
  defaultName?: string;
};

export function ExportDialog({ open, onOpenChange, selectedIds, defaultName }: Props) {
  const [name, setName] = React.useState(defaultName ?? "Track2Mix set");
  const [path, setPath] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [err, setErr] = React.useState<string | null>(null);
  const [ok, setOk] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (open) {
      setName(defaultName ?? "Track2Mix set");
      setErr(null);
      setOk(null);
    }
  }, [open, defaultName]);

  async function pickPath() {
    try {
      const picked = await save({
        defaultPath: path || `${name.replace(/[^a-z0-9\-_ ]/gi, "")}.m3u8`,
        filters: [{ name: "M3U8 playlist", extensions: ["m3u8", "m3u"] }],
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
      const n = await exportPlaylist({
        path,
        playlist_name: name,
        track_ids: selectedIds,
      });
      setOk(`Wrote ${n} track(s) to ${path}`);
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
            {selectedIds.length} track{selectedIds.length === 1 ? "" : "s"} selected.
            Save as M3U8, then in Rekordbox: File → Import Playlist.
          </Dialog.Description>

          <div className="space-y-3">
            <div>
              <label className="text-xs text-muted-foreground mb-1 block">
                Playlist name
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
              <div className="text-xs text-iris-200 bg-iris-500/10 border border-iris-500/25 rounded-md p-2">
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
