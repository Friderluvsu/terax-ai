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
import { fsFor } from "@/modules/workspace";
import { FloppyDiskIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useRef, useState } from "react";
import { resolveSaveAsInput } from "./lib/scratch";
import { settleSaveAsPath, useSaveAsPrompt } from "./lib/saveAsPrompt";

export function SaveAsPathDialog() {
  const request = useSaveAsPrompt((s) => s.request);
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [confirmPath, setConfirmPath] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!request) return;
    setValue(request.name);
    setError(null);
    setConfirmPath(null);
    setChecking(false);
    setTimeout(() => {
      const el = inputRef.current;
      if (!el) return;
      el.focus();
      const dot = el.value.lastIndexOf(".");
      el.setSelectionRange(0, dot > 0 ? dot : el.value.length);
    }, 0);
  }, [request]);

  if (!request) return null;
  const target = resolveSaveAsInput(request.root, value);

  const submit = async () => {
    if (checking) return;
    if ("error" in target) {
      setError(target.error);
      return;
    }
    if (confirmPath === target.path) {
      settleSaveAsPath(target.path);
      return;
    }
    setChecking(true);
    const exists = await fsFor(request.env)
      .stat(target.path)
      .then(
        () => true,
        () => false,
      );
    setChecking(false);
    if (useSaveAsPrompt.getState().request !== request) return;
    if (exists) setConfirmPath(target.path);
    else settleSaveAsPath(target.path);
  };

  const confirming = "path" in target && confirmPath === target.path;

  return (
    <Dialog open onOpenChange={(open) => !open && settleSaveAsPath(null)}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex gap-1.75">
            <HugeiconsIcon icon={FloppyDiskIcon} size={16} strokeWidth={1.75} />
            Save as
          </DialogTitle>
          <DialogDescription>
            Path relative to the space root. The extension determines the
            language mode.
          </DialogDescription>
        </DialogHeader>
        <Input
          ref={inputRef}
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setError(null);
            setConfirmPath(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void submit();
            }
          }}
          placeholder="notes.txt"
        />
        {error ? (
          <div className="text-xs text-destructive">{error}</div>
        ) : confirming ? (
          <div className="text-xs text-destructive">
            This file already exists. Replace it?
          </div>
        ) : (
          <div className="truncate text-xs text-muted-foreground">
            {"path" in target ? target.path : target.error}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={() => settleSaveAsPath(null)}>
            Cancel
          </Button>
          <Button
            variant={confirming ? "destructive" : "default"}
            disabled={checking}
            onClick={() => void submit()}
          >
            {confirming ? "Replace" : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
