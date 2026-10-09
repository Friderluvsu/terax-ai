import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createReloadScheduler, shouldReloadFor } from "./useHtmlPreviewReload";

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ listen: vi.fn() }),
}));

describe("shouldReloadFor", () => {
  it("matches paths at or under the root", () => {
    expect(shouldReloadFor(["/r/a.css"], "/r")).toBe(true);
    expect(shouldReloadFor(["C:\\r\\a.css"], "C:/r")).toBe(true);
    expect(shouldReloadFor(["/other/a.css", "/rx/b"], "/r")).toBe(false);
  });
});

describe("createReloadScheduler", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("collapses bursts into one reload", () => {
    const reload = vi.fn();
    const s = createReloadScheduler(reload, 200);
    s.notify();
    vi.advanceTimersByTime(100);
    s.notify();
    vi.advanceTimersByTime(199);
    expect(reload).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(reload).toHaveBeenCalledTimes(1);
  });

  it("defers while hidden until shown", () => {
    const reload = vi.fn();
    const s = createReloadScheduler(reload, 200, false);
    s.notify();
    vi.advanceTimersByTime(1000);
    expect(reload).not.toHaveBeenCalled();
    s.setVisible(true);
    vi.advanceTimersByTime(200);
    expect(reload).toHaveBeenCalledTimes(1);
  });

  it("does not reload on show without a pending change", () => {
    const reload = vi.fn();
    const s = createReloadScheduler(reload, 200, false);
    s.setVisible(true);
    vi.advanceTimersByTime(1000);
    expect(reload).not.toHaveBeenCalled();
  });
});
