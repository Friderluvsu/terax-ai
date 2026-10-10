import { afterEach, describe, expect, it, vi } from "vitest";
import {
  PTY_RESIZE_INTERVAL_MS,
  PtyResizeScheduler,
} from "./ptyResizeScheduler";

afterEach(() => vi.useRealTimers());

describe("PtyResizeScheduler", () => {
  it("coalesces changes within one interval to the latest dimensions", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);

    scheduler.schedule(80, 24);
    scheduler.schedule(90, 30);
    scheduler.schedule(100, 36);
    vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS - 1);
    expect(deliver).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(deliver).toHaveBeenCalledOnce();
    expect(deliver).toHaveBeenCalledWith(100, 36);
  });

  it("delivers at most once per interval while a resize keeps moving", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);

    for (let i = 0; i < 50; i += 1) {
      scheduler.schedule(80 + i, 24);
      vi.advanceTimersByTime(10);
    }
    vi.runAllTimers();

    expect(deliver.mock.calls.length).toBeGreaterThanOrEqual(5);
    expect(deliver.mock.calls.length).toBeLessThanOrEqual(6);
    expect(deliver).toHaveBeenLastCalledWith(129, 24);
  });

  it("keeps the shell informed during a drag and delivers the end at once", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);

    scheduler.suspend();
    scheduler.schedule(90, 30);
    vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS);
    expect(deliver).toHaveBeenLastCalledWith(90, 30);

    scheduler.schedule(100, 36);
    scheduler.resume();
    expect(deliver).toHaveBeenLastCalledWith(100, 36);
    vi.runAllTimers();
    expect(deliver).toHaveBeenCalledTimes(2);
  });

  it("flushes the final dimensions before a surface detaches", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);

    scheduler.schedule(120, 40);

    expect(scheduler.flush()).toBe(true);
    expect(deliver).toHaveBeenCalledWith(120, 40);
    vi.runAllTimers();
    expect(deliver).toHaveBeenCalledOnce();
  });

  it("does not redeliver unchanged dimensions", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);

    scheduler.schedule(80, 24);
    scheduler.flush();
    scheduler.schedule(80, 24);

    expect(scheduler.flush()).toBe(false);
    vi.runAllTimers();
    expect(deliver).toHaveBeenCalledOnce();
  });

  it("passes a round trip to the shell as a real size change", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);
    scheduler.schedule(80, 24);
    scheduler.flush();
    deliver.mockClear();

    scheduler.schedule(80, 20);
    scheduler.schedule(80, 24);
    vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS);
    expect(deliver.mock.calls).toEqual([[80, 23]]);
    vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS);
    expect(deliver.mock.calls).toEqual([
      [80, 23],
      [80, 24],
    ]);
    vi.runAllTimers();
    expect(deliver).toHaveBeenCalledTimes(2);
  });

  it("redraws after a drag that returns to the start within one interval", () => {
    vi.useFakeTimers();
    const deliver = vi.fn();
    const scheduler = new PtyResizeScheduler(deliver);
    scheduler.schedule(80, 1);
    scheduler.flush();
    deliver.mockClear();

    scheduler.suspend();
    scheduler.schedule(90, 1);
    scheduler.schedule(80, 1);
    scheduler.resume();
    vi.runAllTimers();
    expect(deliver.mock.calls).toEqual([
      [80, 2],
      [80, 1],
    ]);
  });
});
