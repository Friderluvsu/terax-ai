import { describe, expect, it } from "vitest";
import {
  backoffDelay,
  createSshGate,
  reduceConn,
  type SshConnEvent,
} from "./connection";

describe("reduceConn", () => {
  it("walks connecting, connected, lost", () => {
    let s = reduceConn(undefined, "attempt");
    expect(s).toBe("connecting");
    expect(reduceConn(s, "fail")).toBe("connecting");
    s = reduceConn(s, "ok");
    expect(s).toBe("connected");
    s = reduceConn(s, "fail");
    expect(s).toBe("lost");
    expect(reduceConn(s, "attempt")).toBe("connecting");
    expect(reduceConn("connected", "disconnect")).toBe("lost");
  });
});

describe("backoffDelay", () => {
  it("grows and caps", () => {
    expect(backoffDelay(0)).toBe(1000);
    expect(backoffDelay(2)).toBe(4000);
    expect(backoffDelay(10)).toBe(8000);
  });
});

function harness() {
  let connected = false;
  const sleeps: Array<() => void> = [];
  const events: SshConnEvent[] = [];
  const gate = createSshGate({
    check: async () => connected,
    sleep: () => new Promise<void>((r) => sleeps.push(r)),
    emit: (_h, e) => events.push(e),
  });
  const tick = async () => {
    for (let i = 0; i < 20; i++) await Promise.resolve();
  };
  return {
    gate,
    events,
    connect: () => {
      connected = true;
    },
    fire: async () => {
      for (const r of sleeps.splice(0)) r();
      await tick();
    },
    tick,
  };
}

describe("ssh gate", () => {
  it("does not start a second tab before ssh_check succeeds", async () => {
    const h = harness();
    let first = false;
    let second = false;
    void h.gate.enter("box").then(() => (first = true));
    void h.gate.enter("box").then(() => (second = true));
    await h.tick();
    expect(first).toBe(true);
    expect(second).toBe(false);
    await h.fire();
    expect(second).toBe(false);
    h.connect();
    await h.fire();
    expect(second).toBe(true);
    expect(h.events).toContain("ok");
  });

  it("promotes a waiter when the leader gives up", async () => {
    const h = harness();
    const lease = await h.gate.enter("box");
    let second = false;
    void h.gate.enter("box").then(() => (second = true));
    await h.tick();
    expect(second).toBe(false);
    lease.release();
    await h.tick();
    expect(second).toBe(true);
  });

  it("lets everything through when already connected", async () => {
    const h = harness();
    h.connect();
    await h.gate.enter("box");
    await h.gate.enter("box");
    expect(h.events).toEqual(["ok", "ok"]);
  });

  it("gates hosts independently", async () => {
    const h = harness();
    await h.gate.enter("a");
    let b = false;
    void h.gate.enter("b").then(() => (b = true));
    await h.tick();
    expect(b).toBe(true);
  });
});
