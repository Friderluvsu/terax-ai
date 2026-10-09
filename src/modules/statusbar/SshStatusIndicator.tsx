import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useRemoteConnection } from "@/modules/remote/useRemoteConnection";
import { ServerStack03Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

type Props = {
  host: string;
  onReconnect: (host: string) => void;
};

const LABEL = {
  connecting: "connecting",
  connected: "connected",
  lost: "lost",
} as const;

const DOT = {
  connecting: "bg-amber-500",
  connected: "bg-emerald-500",
  lost: "bg-red-500",
} as const;

export default function SshStatusIndicator({ host, onReconnect }: Props) {
  const { state, disconnect } = useRemoteConnection(host);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex h-6 shrink-0 items-center gap-1.5 rounded-sm px-1.5 text-[11px] text-muted-foreground outline-none hover:bg-accent hover:text-foreground data-[state=open]:bg-accent data-[state=open]:text-foreground"
          title={`SSH ${LABEL[state]}`}
        >
          <HugeiconsIcon
            icon={ServerStack03Icon}
            size={13}
            strokeWidth={1.75}
          />
          <span className="max-w-32 truncate">SSH: {host}</span>
          <span className={`size-1.5 rounded-full ${DOT[state]}`} />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-40">
        <DropdownMenuItem disabled>{LABEL[state]}</DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onReconnect(host)}>
          Reconnect
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={state === "lost"}
          onSelect={() => void disconnect()}
        >
          Disconnect
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
