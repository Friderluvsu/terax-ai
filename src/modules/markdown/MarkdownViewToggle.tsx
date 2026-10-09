import { DocumentViewToggle } from "./DocumentViewToggle";

type Mode = "rendered" | "raw";

type Props = {
  mode: Mode;
  onChange: (mode: Mode) => void;
  renderedDisabled?: boolean;
  renderedHint?: string;
};

export function MarkdownViewToggle({
  mode,
  onChange,
  renderedDisabled,
  renderedHint,
}: Props) {
  return (
    <DocumentViewToggle
      mode={mode}
      onChange={onChange}
      options={[
        {
          value: "rendered",
          label: "Rendered",
          disabled: renderedDisabled,
          hint: renderedHint,
        },
        { value: "raw", label: "Raw" },
      ]}
    />
  );
}
