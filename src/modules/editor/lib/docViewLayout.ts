import type { DocView } from "@/modules/tabs";

export type DocViewLayout = {
  editor: number;
  preview: number;
  showPreview: boolean;
};

export function docViewLayout(view: DocView | undefined): DocViewLayout {
  switch (view) {
    case "split":
      return { editor: 50, preview: 50, showPreview: true };
    case "preview":
      return { editor: 0, preview: 100, showPreview: true };
    default:
      return { editor: 100, preview: 0, showPreview: false };
  }
}
