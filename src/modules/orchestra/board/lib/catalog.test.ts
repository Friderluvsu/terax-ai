import { describe, expect, it } from "vitest";
import {
  type AgentCatalog,
  CUSTOM_MODEL,
  isInstalled,
  modelChoice,
  modelsFor,
} from "./catalog";

const catalog: AgentCatalog[] = [
  {
    agent: "claude",
    installed: true,
    models: [{ id: "haiku", label: "Haiku", note: "cheap" }],
  },
  { agent: "antigravity", installed: false, models: [] },
];

describe("agent catalog helpers", () => {
  it("lists models per agent and flags missing agents", () => {
    expect(modelsFor(catalog, "claude").map((m) => m.id)).toEqual(["haiku"]);
    expect(modelsFor(catalog, "pi")).toEqual([]);
    expect(isInstalled(catalog, "claude")).toBe(true);
    expect(isInstalled(catalog, "antigravity")).toBe(false);
    expect(isInstalled([], "antigravity")).toBe(true);
  });

  it("maps a stored model onto the select", () => {
    const models = modelsFor(catalog, "claude");
    expect(modelChoice(models, "")).toBe("");
    expect(modelChoice(models, "haiku")).toBe("haiku");
    expect(modelChoice(models, "o3")).toBe(CUSTOM_MODEL);
  });
});
