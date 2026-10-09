import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { usePreferencesStore } from "@/modules/settings/preferences";
import { Switch } from "@/components/ui/switch";
import {
  setTerminalClickToMoveCursor,
  setLaunchCommandFiles,
  setTerminalDefaultMode,
  setTerminalOptionAsMeta,
  type TerminalOptionAsMeta,
  type TerminalDefaultMode,
} from "@/modules/settings/store";
import { SectionHeader } from "../components/SectionHeader";
import { SettingRow } from "../components/SettingRow";

const NEW_TAB_MODES: { value: TerminalDefaultMode; label: string }[] = [
  { value: "classic", label: "Classic" },
  { value: "blocks", label: "Blocks" },
];

export function ForkSection() {
  const newTabMode = usePreferencesStore((s) => s.terminalDefaultMode);
  const launchCommandFiles = usePreferencesStore((s) => s.launchCommandFiles);
  const optionAsMeta = usePreferencesStore((s) => s.terminalOptionAsMeta);
  const clickToMoveCursor = usePreferencesStore(
    (s) => s.terminalClickToMoveCursor,
  );

  return (
    <div className="flex flex-col gap-6">
      <SectionHeader title="Fork" description="Fork-only behavior." />

      <div className="flex flex-col gap-2">
        <SettingRow
          title="New tab mode"
          description="Cmd+T opens this kind of tab and Cmd+Shift+T opens the other. Blocks tabs cannot be split into panes; when Blocks is the default, use Cmd+Shift+T to open a Classic tab for panes."
        >
          <Select
            value={newTabMode}
            onValueChange={(v) =>
              void setTerminalDefaultMode(v as TerminalDefaultMode)
            }
          >
            <SelectTrigger value={newTabMode} className="h-8 w-28 text-[12px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {NEW_TAB_MODES.map((m) => (
                <SelectItem
                  key={m.value}
                  value={m.value}
                  className="text-[12px]"
                >
                  {m.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SettingRow>
        <SettingRow
          title="Command files from Finder"
          description="What happens when a .command or .tool file is opened with Terax from Finder."
        >
          <Select
            value={launchCommandFiles}
            onValueChange={(value) =>
              void setLaunchCommandFiles(value === "edit" ? "edit" : "run")
            }
          >
            <SelectTrigger className="h-8 w-36 text-[12px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="run">Run in terminal</SelectItem>
              <SelectItem value="edit">Open in editor</SelectItem>
            </SelectContent>
          </Select>
        </SettingRow>
        <SettingRow
          title="Click to move cursor"
          description="Click in the command line of a Classic tab to move the cursor there. Hold Option to move it without shell integration. Cmd+click opens a link inside the command line."
        >
          <Switch
            checked={clickToMoveCursor}
            onCheckedChange={(v) => void setTerminalClickToMoveCursor(v)}
          />
        </SettingRow>
        <SettingRow
          title="Option as Meta"
          description="Option+B, F and D act as Meta (Esc b, f, d) in the terminal. Left only keeps the right Option for typing characters such as guillemets and the em dash."
        >
          <Select
            value={optionAsMeta}
            onValueChange={(v) =>
              void setTerminalOptionAsMeta(v as TerminalOptionAsMeta)
            }
          >
            <SelectTrigger className="h-8 w-36 text-[12px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="off">Off</SelectItem>
              <SelectItem value="left">Left Option</SelectItem>
              <SelectItem value="both">Both Options</SelectItem>
            </SelectContent>
          </Select>
        </SettingRow>
      </div>
    </div>
  );
}
