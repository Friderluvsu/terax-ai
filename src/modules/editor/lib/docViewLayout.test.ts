import { describe, expect, it } from "vitest";
import { docViewLayout } from "./docViewLayout";

describe("docViewLayout", () => {
  it("defaults to code with the preview panel empty", () => {
    expect(docViewLayout(undefined)).toEqual({
      editor: 100,
      preview: 0,
      showPreview: false,
    });
    expect(docViewLayout("code")).toEqual(docViewLayout(undefined));
  });

  it("splits evenly and hides the editor only in preview", () => {
    expect(docViewLayout("split")).toEqual({
      editor: 50,
      preview: 50,
      showPreview: true,
    });
    expect(docViewLayout("preview")).toEqual({
      editor: 0,
      preview: 100,
      showPreview: true,
    });
  });

  it("always sums to 100 for any transition", () => {
    for (const v of ["code", "split", "preview", undefined] as const) {
      const l = docViewLayout(v);
      expect(l.editor + l.preview).toBe(100);
    }
  });
});
