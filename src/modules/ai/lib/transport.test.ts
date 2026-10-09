import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UIMessage } from "@ai-sdk/react";

const invoke = vi.hoisted(() => vi.fn());
const runAgentStream = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@/modules/settings/store", () => ({ setLastWslDistro: vi.fn() }));
vi.mock("./agent", () => ({ runAgentStream }));

import type { WorkspaceEnv } from "@/modules/workspace";
import type { BaseToolContext } from "./runContext";
import { createContextAwareTransport } from "./transport";

const user = { id: "u", role: "user", parts: [{ type: "text", text: "hi" }] };
const assistant = { id: "a", role: "assistant", parts: [] };

describe("transport run env binding", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({});
    runAgentStream.mockReset();
    runAgentStream.mockResolvedValue({ toUIMessageStream: () => ({}) });
  });

  it("approval continuation keeps the env of the originating turn", async () => {
    const wsl: WorkspaceEnv = { kind: "wsl", distro: "Ubuntu" };
    let active: WorkspaceEnv = wsl;
    const transport = createContextAwareTransport({
      getKeys: () => ({}),
      toolContext: { readCache: new Map() } as unknown as BaseToolContext,
      getEnv: () => active,
      getModelId: () => "m",
      getCustomInstructions: () => "",
      getAgentPersona: () => null,
      getLive: () => ({
        cwd: null,
        terminalPrivate: false,
        workspaceRoot: null,
        activeFile: null,
      }),
    } as never);

    await transport.sendMessages({ messages: [user] as UIMessage[] });
    active = { kind: "local" };
    await transport.sendMessages({
      messages: [user, assistant] as UIMessage[],
    });

    const [first, second] = runAgentStream.mock.calls.map((c) => c[0]);
    expect(first.toolContext.env).toEqual(wsl);
    expect(second.toolContext.env).toEqual(wsl);
    expect(transport.getRunEnv()).toEqual(wsl);
    await second.toolContext.native.writeFile("/home/u/a.txt", "x");
    const call = invoke.mock.calls.find((c) => c[1]?.workspace);
    expect(call?.[1].workspace).toEqual(wsl);

    // A new user turn re-binds to the now-active space.
    await transport.sendMessages({
      messages: [user, assistant, { ...user, id: "u2" }] as UIMessage[],
    });
    expect(runAgentStream.mock.calls[2][0].toolContext.env).toEqual({
      kind: "local",
    });
  });
});
