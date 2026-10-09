import { lazy, Suspense } from "react";
import type { ComponentProps } from "react";
import { envCaps, useWorkspaceEnvStore } from "@/modules/workspace";
import type { SourceControlPanel as SourceControlPanelType } from "./SourceControlPanel";

const SourceControlPanelInner = lazy(() =>
  import("./SourceControlPanel").then((m) => ({
    default: m.SourceControlPanel,
  })),
);

type Props = ComponentProps<typeof SourceControlPanelType>;

export function SourceControlPanel(props: Props) {
  const gitAvailable = envCaps(useWorkspaceEnvStore((s) => s.env)).git;
  if (!gitAvailable) {
    return (
      <div className="flex h-full items-center justify-center px-6 text-center text-xs text-muted-foreground">
        Git is not available for SSH, use the terminal.
      </div>
    );
  }
  return (
    <Suspense fallback={null}>
      <SourceControlPanelInner {...props} />
    </Suspense>
  );
}
