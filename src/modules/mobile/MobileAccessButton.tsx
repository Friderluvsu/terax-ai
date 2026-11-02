import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { SmartPhone01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { listen } from "@tauri-apps/api/event";
import { lazy, Suspense, useEffect, useState } from "react";
import {
  MOBILE_OFF,
  MOBILE_STATUS_EVENT,
  type MobileStatus,
} from "./lib/status";

const MobileAccessPanel = lazy(() => import("./MobileAccessPanel"));

export function MobileAccessButton() {
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<MobileStatus>(MOBILE_OFF);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<MobileStatus>(MOBILE_STATUS_EVENT, ({ payload }) =>
      setStatus(payload),
    ).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const live = status.running && status.tunnel.state === "ready";

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          size="icon-sm"
          variant="ghost"
          title={live ? "Phone access is on" : "Phone access"}
          className={cn(
            "relative shrink-0 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground",
            status.running && "text-foreground",
          )}
        >
          <HugeiconsIcon icon={SmartPhone01Icon} size={16} strokeWidth={1.75} />
          {status.running && (
            <span
              className={cn(
                "absolute top-1 right-1 size-1.5 rounded-full",
                live ? "bg-emerald-500" : "bg-amber-500",
              )}
            />
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 gap-3">
        {open && (
          <Suspense fallback={<div className="h-40" />}>
            <MobileAccessPanel status={status} onStatus={setStatus} />
          </Suspense>
        )}
      </PopoverContent>
    </Popover>
  );
}
