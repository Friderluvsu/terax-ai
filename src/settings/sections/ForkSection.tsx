import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { usePreferencesStore } from "@/modules/settings/preferences";
import { setLaunchCommandFiles } from "@/modules/settings/store";
import { SectionHeader } from "../components/SectionHeader";
import { SettingRow } from "../components/SettingRow";

export function ForkSection() {
  const launchCommandFiles = usePreferencesStore((s) => s.launchCommandFiles);
  return (
    <div className="flex flex-col gap-4">
      <SectionHeader title="Fork" />
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
    </div>
  );
}
