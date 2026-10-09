import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useState } from "react";
import { sshHostDetail } from "./hosts";
import { useSshHosts } from "./useSshHosts";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConnect: (alias: string, path: string | null) => void;
};

export default function ConnectDialog({
  open,
  onOpenChange,
  onConnect,
}: Props) {
  const hosts = useSshHosts(open);
  const [selected, setSelected] = useState<string | null>(null);
  const [path, setPath] = useState("");
  const alias = selected ?? hosts[0]?.alias ?? null;

  const submit = () => {
    if (!alias) return;
    const trimmed = path.trim();
    onConnect(alias, trimmed.startsWith("/") ? trimmed : null);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Connect to SSH host</DialogTitle>
          <DialogDescription>
            Opens a remote space. Sign in from the first terminal tab.
          </DialogDescription>
        </DialogHeader>
        <div className="max-h-56 overflow-y-auto rounded-md border border-border/60">
          {hosts.length === 0 ? (
            <div className="px-3 py-2 text-xs text-muted-foreground">
              No hosts in ~/.ssh/config
            </div>
          ) : (
            hosts.map((host) => (
              <button
                key={host.alias}
                type="button"
                onClick={() => setSelected(host.alias)}
                className={`flex w-full items-center justify-between gap-3 px-3 py-1.5 text-left text-xs hover:bg-accent/60 ${
                  host.alias === alias ? "bg-accent" : ""
                }`}
              >
                <span>{host.alias}</span>
                <span className="truncate text-muted-foreground">
                  {sshHostDetail(host)}
                </span>
              </button>
            ))
          )}
        </div>
        <Input
          value={path}
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
          }}
          placeholder="Remote path (default: home)"
          spellCheck={false}
        />
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={!alias}>
            Connect
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
