import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import { fmtShortcut, MOD_KEY, SHIFT_KEY } from "@/lib/platform";
import { AgentLauncherPanel } from "@/modules/agents/components/AgentLauncherPanel";
import type { AgentLaunchRequest } from "@/modules/agents/lib/launcher";
import { useSshHosts } from "@/modules/remote";
import {
  AiBrowserIcon,
  ArrowRight01Icon,
  ComputerTerminal02Icon,
  GitBranchIcon,
  Globe02Icon,
  IncognitoIcon,
  PencilEdit02Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useRef, useState } from "react";

type Props = {
  onNew: () => void;
  onNewBlock: () => void;
  onNewPrivate: () => void;
  onNewPreview: () => void;
  onNewEditor: () => void;
  onNewGitGraph: () => void;
  onNewSsh: (alias: string) => void;
  onLaunchAgents: (request: AgentLaunchRequest) => void;
};

export function NewTabMenu({
  onNew,
  onNewBlock,
  onNewPrivate,
  onNewPreview,
  onNewEditor,
  onNewGitGraph,
  onNewSsh,
  onLaunchAgents,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const sshHosts = useSshHosts(menuOpen);
  const [launcherOpen, setLauncherOpen] = useState(false);
  const openLauncherAfterMenuClose = useRef(false);
  const openMenuAfterLauncherClose = useRef(false);

  const onMenuOpenChange = (next: boolean) => {
    if (next) {
      openLauncherAfterMenuClose.current = false;
      setLauncherOpen(false);
    }
    setMenuOpen(next);
  };

  const openLauncher = () => {
    openLauncherAfterMenuClose.current = true;
  };

  const backToMenu = () => {
    openMenuAfterLauncherClose.current = true;
    setLauncherOpen(false);
  };

  return (
    <Popover open={launcherOpen} onOpenChange={setLauncherOpen}>
      <PopoverAnchor asChild>
        <span className="inline-flex">
          <DropdownMenu open={menuOpen} onOpenChange={onMenuOpenChange}>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="ml-1 size-6 shrink-0 rounded-full bg-foreground/[0.06] text-muted-foreground ring-1 ring-inset ring-foreground/[0.04] hover:bg-foreground/[0.12] hover:text-foreground"
                title="New tab"
              >
                <HugeiconsIcon icon={PlusSignIcon} size={14} strokeWidth={2} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              align="start"
              className="min-w-44"
              onCloseAutoFocus={(event) => {
                event.preventDefault();
                if (!openLauncherAfterMenuClose.current) return;

                openLauncherAfterMenuClose.current = false;
                requestAnimationFrame(() => setLauncherOpen(true));
              }}
            >
              <DropdownMenuItem onSelect={onNew}>
                <HugeiconsIcon
                  icon={ComputerTerminal02Icon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Terminal</span>
                <span className="text-xs text-muted-foreground">
                  {fmtShortcut(MOD_KEY, "T")}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onNewBlock}>
                <HugeiconsIcon
                  icon={ComputerTerminal02Icon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Blocks</span>
                <span className="text-xs text-muted-foreground">
                  {fmtShortcut(MOD_KEY, SHIFT_KEY, "T")}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={openLauncher}>
                <HugeiconsIcon
                  icon={AiBrowserIcon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Agents</span>
                <HugeiconsIcon
                  icon={ArrowRight01Icon}
                  size={14}
                  strokeWidth={1.75}
                  className="text-muted-foreground"
                />
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onNewPrivate}>
                <HugeiconsIcon
                  icon={IncognitoIcon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Privacy</span>
                <span className="text-xs text-muted-foreground">
                  {fmtShortcut(MOD_KEY, "R")}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onNewEditor}>
                <HugeiconsIcon
                  icon={PencilEdit02Icon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Editor</span>
                <span className="text-xs text-muted-foreground">
                  {fmtShortcut(MOD_KEY, "E")}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onNewPreview}>
                <HugeiconsIcon
                  icon={Globe02Icon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Preview</span>
                <span className="text-xs text-muted-foreground">
                  {fmtShortcut(MOD_KEY, "P")}
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={onNewGitGraph}>
                <HugeiconsIcon
                  icon={GitBranchIcon}
                  size={14}
                  strokeWidth={1.75}
                />
                <span className="flex-1">Git Graph</span>
              </DropdownMenuItem>
              {sshHosts.length > 0 && (
                <DropdownMenuSub>
                  <DropdownMenuSubTrigger>
                    <HugeiconsIcon
                      icon={ComputerTerminal02Icon}
                      size={14}
                      strokeWidth={1.75}
                    />
                    <span className="flex-1">SSH</span>
                  </DropdownMenuSubTrigger>
                  <DropdownMenuSubContent className="max-h-72 min-w-40 overflow-y-auto">
                    {sshHosts.map((host) => (
                      <DropdownMenuItem
                        key={host.alias}
                        onSelect={() => onNewSsh(host.alias)}
                      >
                        {host.alias}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuSubContent>
                </DropdownMenuSub>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </span>
      </PopoverAnchor>
      <PopoverContent
        align="start"
        sideOffset={6}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          if (!openMenuAfterLauncherClose.current) return;

          openMenuAfterLauncherClose.current = false;
          requestAnimationFrame(() => setMenuOpen(true));
        }}
        className="w-[340px] gap-0 overflow-hidden rounded-2xl p-1.5"
      >
        <AgentLauncherPanel
          onBack={backToMenu}
          onLaunch={(request) => {
            setLauncherOpen(false);
            onLaunchAgents(request);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
