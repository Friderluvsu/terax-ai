import { cn } from "@/lib/utils";
import { isRunnableScriptPath } from "@/modules/terminal/lib/runScript";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type Props = {
  path: string;
  dirty: boolean;
  onRun: (path: string) => void;
};

function useShebang(path: string, dirty: boolean, enabled: boolean): boolean {
  const [found, setFound] = useState(false);
  useEffect(() => {
    if (!enabled) {
      setFound(false);
      return;
    }
    if (dirty) return;
    let cancelled = false;
    invoke<{ hasShebang: boolean }>("script_launch_info", { path })
      .then((info) => {
        if (!cancelled) setFound(info.hasShebang);
      })
      .catch(() => {
        if (!cancelled) setFound(false);
      });
    return () => {
      cancelled = true;
    };
  }, [path, dirty, enabled]);
  return found;
}

export function RunScriptButton({ path, dirty, onRun }: Props) {
  const byExt = isRunnableScriptPath(path);
  const byShebang = useShebang(path, dirty, !byExt);
  if (!byExt && !byShebang) return null;
  return (
    <div className="absolute right-3 top-3 z-10 inline-flex items-center rounded-md border border-border/60 bg-card/85 p-0.5 text-[11px] shadow-sm backdrop-blur">
      <button
        type="button"
        onClick={() => onRun(path)}
        disabled={dirty}
        title={dirty ? "Save to run" : "Run in Terminal"}
        className={cn(
          "rounded px-2 py-0.5 text-muted-foreground transition-colors hover:text-foreground",
          dirty && "cursor-not-allowed opacity-40 hover:text-muted-foreground",
        )}
      >
        Run
      </button>
    </div>
  );
}
