import { describe, expect, it } from "vitest";
import { createParams, draftProblem, newDraft, parsePlan } from "./drafts";

describe("drafts", () => {
  it("keeps only well-formed planned tasks", () => {
    expect(
      parsePlan([{ title: "A", brief: "do a" }, { title: 1 }, null]),
    ).toEqual([{ title: "A", brief: "do a" }]);
    expect(() => parsePlan({})).toThrow();
  });

  it("keeps the planner's agent and model only when they are valid", () => {
    const [first, second, third] = parsePlan([
      { title: "A", brief: "a", agent: "codex", model: "gpt-x" },
      { title: "B", brief: "b", agent: "nope", model: "gpt-x" },
      { title: "C", brief: "c", agent: "claude", model: "--bad" },
    ]);
    expect(first).toMatchObject({ agent: "codex", model: "gpt-x" });
    expect(second.agent).toBeUndefined();
    expect(second.model).toBeUndefined();
    expect(third).toMatchObject({ agent: "claude" });
    expect(third.model).toBeUndefined();
    expect(newDraft("antigravity", first)).toMatchObject({
      agent: "codex",
      model: "gpt-x",
    });
    expect(newDraft("antigravity", second).agent).toBe("antigravity");
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
    const draft = newDraft("antigravity", { title: " ", brief: " fix it " });
    expect(createParams(draft)).toEqual({
      agent: "antigravity",
      brief: "fix it",
      title: null,
      model: null,
    });
    expect(newDraft("claude").id).not.toBe(draft.id);
  });
});
