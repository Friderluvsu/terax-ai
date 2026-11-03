import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
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
  DashboardSquare01Icon,
  ServerStack01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type ComponentProps, useRef, useState } from "react";

type Props = {
  onNew: () => void;
  onNewBlock: () => void;
  onNewPrivate: () => void;
  onNewPreview: () => void;
  onNewEditor: () => void;
  onNewGitGraph: () => void;
  onNewSsh: (alias: string) => void;
  onConnectSsh: () => void;
  onLaunchAgents: (request: AgentLaunchRequest) => void;
  onNewTaskBoard: () => void;
};

export function NewTabMenu({
  onNew,
  onNewBlock,
  onNewPrivate,
  onNewPreview,
  onNewEditor,
  onNewGitGraph,
  onNewSsh,
  onConnectSsh,
  onLaunchAgents,
  onNewTaskBoard,
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
              className="min-w-48"
              onCloseAutoFocus={(event) => {
                event.preventDefault();
                if (!openLauncherAfterMenuClose.current) return;

                openLauncherAfterMenuClose.current = false;
                requestAnimationFrame(() => setLauncherOpen(true));
              }}
            >
              <Item
                icon={ComputerTerminal02Icon}
                label="Terminal"
                onSelect={onNew}
                keys={fmtShortcut(MOD_KEY, "T")}
              />
              <Item
                icon={ComputerTerminal02Icon}
                label="Blocks"
                onSelect={onNewBlock}
                keys={fmtShortcut(MOD_KEY, SHIFT_KEY, "T")}
              />
              <Item
                icon={IncognitoIcon}
                label="Privacy"
                onSelect={onNewPrivate}
                keys={fmtShortcut(MOD_KEY, "R")}
              />
              <Separator />
              <Item
                icon={AiBrowserIcon}
                label="Agents"
                onSelect={openLauncher}
                more
              />
              <Item
                icon={DashboardSquare01Icon}
                label="Task board"
                onSelect={onNewTaskBoard}
              />
              <Separator />
              <Item
                icon={PencilEdit02Icon}
                label="Editor"
                onSelect={onNewEditor}
                keys={fmtShortcut(MOD_KEY, "E")}
              />
              <Item
                icon={Globe02Icon}
                label="Preview"
                onSelect={onNewPreview}
                keys={fmtShortcut(MOD_KEY, "P")}
              />
              <Item
                icon={GitBranchIcon}
                label="Git Graph"
                onSelect={onNewGitGraph}
              />
              <Separator />
              {sshHosts.length > 0 ? (
                <DropdownMenuSub>
                  <DropdownMenuSubTrigger>
                    <HugeiconsIcon
                      icon={ServerStack01Icon}
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
                    <Separator />
                    <DropdownMenuItem onSelect={onConnectSsh}>
                      Connect...
                    </DropdownMenuItem>
                  </DropdownMenuSubContent>
                </DropdownMenuSub>
              ) : (
                <Item
                  icon={ServerStack01Icon}
                  label="SSH..."
                  onSelect={onConnectSsh}
                />
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

function Separator() {
  return <DropdownMenuSeparator className="my-1 border-t border-border/30" />;
}

function Item({
  icon,
  label,
  onSelect,
  keys,
  more,
}: {
  icon: ComponentProps<typeof HugeiconsIcon>["icon"];
  label: string;
  onSelect: () => void;
  keys?: string;
  more?: boolean;
}) {
  return (
    <DropdownMenuItem onSelect={onSelect}>
      <HugeiconsIcon icon={icon} size={14} strokeWidth={1.75} />
      <span className="flex-1">{label}</span>
      {keys ? (
        <span className="text-xs text-muted-foreground">{keys}</span>
      ) : null}
      {more ? (
        <HugeiconsIcon
          icon={ArrowRight01Icon}
          size={14}
          strokeWidth={1.75}
          className="text-muted-foreground"
        />
      ) : null}
    </DropdownMenuItem>
  );
}
