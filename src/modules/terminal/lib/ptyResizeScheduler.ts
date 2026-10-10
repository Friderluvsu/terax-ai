export const PTY_RESIZE_INTERVAL_MS = 100;

export type PtyDimensions = {
  readonly cols: number;
  readonly rows: number;
};

export type PtyResizeSchedulerDiagnostics = {
  readonly schedules: number;
  readonly deliveries: number;
  readonly suspended: boolean;
  readonly timerScheduled: boolean;
  readonly pending: PtyDimensions | null;
  readonly delivered: PtyDimensions | null;
};

/**
 * Throttles PTY geometry delivery independently from a renderer surface. The
 * model reflows immediately and clears its prompt, so the shell must hear about
 * sizes during a drag to redraw it; at most one delivery per interval keeps
 * full-screen redraws bounded. A replaced PTY generation can reset stale state.
 */
export class PtyResizeScheduler {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private pending: PtyDimensions | null = null;
  private delivered: PtyDimensions | null = null;
  private moved = false;
  private suspended = false;
  private scheduleCount = 0;
  private deliveryCount = 0;

  constructor(
    private readonly deliver: (cols: number, rows: number) => void,
    private readonly intervalMs = PTY_RESIZE_INTERVAL_MS,
  ) {}

  schedule(cols: number, rows: number): void {
    if (
      !Number.isInteger(cols) ||
      !Number.isInteger(rows) ||
      cols < 1 ||
      rows < 1
    ) {
      throw new RangeError("PTY resize dimensions must be positive integers");
    }
    this.scheduleCount += 1;
    if (!sameDimensions(this.pending, cols, rows)) {
      this.pending = { cols, rows };
    }
    if (!sameDimensions(this.delivered, cols, rows)) this.moved = true;
    if (this.timer === null) this.armTimer();
  }

  /** Marks a user drag; deliveries stay throttled while it lasts. */
  suspend(): void {
    this.suspended = true;
  }

  /** Ends a user drag and delivers the final dimensions without waiting. */
  resume(): void {
    if (!this.suspended) return;
    this.suspended = false;
    if (this.pending !== null) this.flush();
  }

  /** Clears dimensions belonging to a PTY generation that is being replaced. */
  reset(): void {
    if (this.timer !== null) globalThis.clearTimeout(this.timer);
    this.timer = null;
    this.pending = null;
    this.delivered = null;
    this.moved = false;
  }

  flush(): boolean {
    if (this.timer !== null) globalThis.clearTimeout(this.timer);
    this.timer = null;
    const dimensions = this.pending;
    const moved = this.moved;
    this.pending = null;
    this.moved = false;
    if (!dimensions) return false;
    if (sameDimensions(this.delivered, dimensions.cols, dimensions.rows)) {
      if (!moved) return false;
      // The terminal reflowed and came back, clearing its prompt; the shell
      // redraws only when it observes a different size, so pass through one.
      const { cols, rows } = dimensions;
      this.send({ cols, rows: rows > 1 ? rows - 1 : rows + 1 });
      this.pending = dimensions;
      this.armTimer();
      return true;
    }
    this.send(dimensions);
    return true;
  }

  diagnostics(): PtyResizeSchedulerDiagnostics {
    return {
      schedules: this.scheduleCount,
      deliveries: this.deliveryCount,
      suspended: this.suspended,
      timerScheduled: this.timer !== null,
      pending: this.pending ? { ...this.pending } : null,
      delivered: this.delivered ? { ...this.delivered } : null,
    };
  }

  private send(dimensions: PtyDimensions): void {
    this.delivered = dimensions;
    this.deliveryCount += 1;
    this.deliver(dimensions.cols, dimensions.rows);
  }

  private armTimer(): void {
    if (this.timer !== null) globalThis.clearTimeout(this.timer);
    this.timer = globalThis.setTimeout(() => this.flush(), this.intervalMs);
  }
}

function sameDimensions(
  dimensions: PtyDimensions | null,
  cols: number,
  rows: number,
): boolean {
  return dimensions?.cols === cols && dimensions.rows === rows;
}
