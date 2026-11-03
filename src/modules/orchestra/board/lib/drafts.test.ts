import { describe, expect, it } from "vitest";
import { createParams, draftProblem, newDraft, parsePlan } from "./drafts";

describe("drafts", () => {
  it("keeps only well-formed planned tasks", () => {
    expect(
      parsePlan([{ title: "A", brief: "do a" }, { title: 1 }, null]),
    ).toEqual([{ title: "A", brief: "do a" }]);
    expect(() => parsePlan({})).toThrow();
  });

  it("flags cards the backend would refuse", () => {
    const draft = newDraft("codex", { title: "Docs", brief: "write docs" });
    expect(draftProblem(draft)).toBeNull();
    expect(draftProblem({ ...draft, brief: "  " })).toMatch(/Describe/);
    expect(draftProblem({ ...draft, model: "--x" })).toMatch(/model/);
    expect(draftProblem({ ...draft, title: "a\nb" })).toMatch(/title/);
    expect(draftProblem({ ...draft, brief: "x".repeat(17 * 1024) })).toMatch(
      /16 KB/,
    );
  });

  it("sends trimmed fields and omits empty options", () => {
    const draft = newDraft("gemini", { title: " ", brief: " fix it " });
    expect(createParams(draft)).toEqual({
      agent: "gemini",
      brief: "fix it",
      title: null,
      model: null,
    });
    expect(newDraft("claude").id).not.toBe(draft.id);
  });
});
