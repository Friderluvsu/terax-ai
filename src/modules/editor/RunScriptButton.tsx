import { cn } from "@/lib/utils";
import {
  hasShebang,
  isRunnableScriptPath,
} from "@/modules/terminal/lib/runScript";
import { currentWorkspaceEnv } from "@/modules/workspace";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type Props = {
  path: string;
  dirty: boolean;
  onRun: (path: string) => void;
};

function hasExtension(path: string): boolean {
  const name = path.slice(path.lastIndexOf("/") + 1);
  return name.lastIndexOf(".") > 0;
}

function useShebang(path: string, enabled: boolean): boolean {
  const [found, setFound] = useState(false);
  useEffect(() => {
    setFound(false);
    if (!enabled) return;
    let cancelled = false;
    invoke<{ kind: string; content?: string }>("fs_read_file", {
      path,
      workspace: currentWorkspaceEnv(),
    })
      .then((res) => {
        if (!cancelled && res.kind === "text" && res.content) {
          setFound(hasShebang(res.content));
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [path, enabled]);
  return found;
}

export function RunScriptButton({ path, dirty, onRun }: Props) {
  const byExt = isRunnableScriptPath(path);
  const byShebang = useShebang(path, !byExt && !hasExtension(path));
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
