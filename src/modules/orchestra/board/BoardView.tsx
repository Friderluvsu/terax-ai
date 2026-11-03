import { MinusSignIcon, PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import type { BoardTask } from "../lib/board";
import { useOrchestraTasks } from "../lib/useOrchestraTasks";
import {
  DEFAULT_GRID,
  fitView,
  type Grid,
  moveRect,
  placeTile,
  type Rect,
  resizeGrid,
  type Size,
  tileSize,
  toScreen,
  type View,
  zoomAt,
} from "./lib/canvas";
import { PlannerBar } from "./PlannerBar";
import { useBoard, useBoardStore } from "./store";
import { OffCanvasTasks, TileHeader } from "./Tile";

// Terminal rendering comes in through props so this lazy chunk never pulls
// terminal modules out of their own chunk.
type Props = {
  run: string;
  cwd: string;
  leaves: number[];
  activeLeafId: number;
  renderLeaf: (leafId: number, visible: boolean) => ReactNode;
  onResizing: (token: object, active: boolean) => void;
  useLeafCell: (leafId: number) => Cell | null;
  setLeafFontScale: (leafId: number, scale: number) => void;
  writeToLeaf: (leafId: number, text: string) => boolean;
  whenLeafReady: (leafId: number) => Promise<void>;
};

type Cell = { width: number; height: number; fontSize: number };

const FALLBACK_CELL = { width: 8, height: 17 };
const ZOOM_SETTLE_MS = 350;

const ZOOM_STEP = 1.15;

function intersects(rect: Rect, viewport: Size): boolean {
  return (
    rect.x < viewport.w &&
    rect.y < viewport.h &&
    rect.x + rect.w > 0 &&
    rect.y + rect.h > 0
  );
}

/** Pointer drag that reports movement since the press. */
function drag(
  event: React.PointerEvent,
  onMove: (dx: number, dy: number) => void,
  onEnd?: () => void,
) {
  if (event.button !== 0) return;
  event.preventDefault();
  const target = event.currentTarget as HTMLElement;
  target.setPointerCapture(event.pointerId);
  const startX = event.clientX;
  const startY = event.clientY;
  const move = (e: PointerEvent) =>
    onMove(e.clientX - startX, e.clientY - startY);
  const up = () => {
    target.removeEventListener("pointermove", move);
    target.removeEventListener("pointerup", up);
    target.removeEventListener("pointercancel", up);
    onEnd?.();
  };
  target.addEventListener("pointermove", move);
  target.addEventListener("pointerup", up);
  target.addEventListener("pointercancel", up);
}

export default function BoardView(props: Props) {
  const { run, cwd, activeLeafId, renderLeaf, onResizing, setLeafFontScale } =
    props;
  const leafKey = props.leaves.join(",");
  // biome-ignore lint/correctness/useExhaustiveDependencies: keyed by content
  const leaves = useMemo(() => props.leaves, [leafKey]);
  const board = useBoard(run);
  const update = useBoardStore((state) => state.update);
  const tasks = useOrchestraTasks();
  const consoleLeaf = leaves[0];
  const runTasks = useMemo(
    () => tasks.filter((task) => task.run === run),
    [tasks, run],
  );
  const taskByLeaf = useMemo(() => {
    const map = new Map<number, BoardTask>();
    for (const task of runTasks) {
      if (task.pane_id !== undefined) map.set(task.pane_id, task);
    }
    return map;
  }, [runTasks]);

  const canvasRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState<Size>({ w: 0, h: 0 });
  useEffect(() => {
    const el = canvasRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) =>
      setViewport({ w: entry.contentRect.width, h: entry.contentRect.height }),
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    update(run, (b) => {
      const kept: Record<number, Rect> = {};
      const grids: Record<number, Grid> = {};
      let changed = false;
      for (const id of leaves) {
        const rect = b.rects[id] ?? placeTile(Object.values(kept));
        if (!b.rects[id] || !b.grids[id]) changed = true;
        kept[id] = rect;
        grids[id] = b.grids[id] ?? DEFAULT_GRID;
      }
      if (Object.keys(b.rects).length !== leaves.length) changed = true;
      return changed ? { rects: kept, grids } : {};
    });
  }, [run, leaves, update]);

  // Zoom scales every terminal's font and keeps its rows and columns, so text
  // never reflows; the resize interaction holds PTY resizes until the new
  // cell size has been applied to the tile.
  const zoom = board.view.zoom;
  const appliedZoom = useRef(zoom);
  const zoomToken = useRef({});
  useEffect(() => {
    const token = zoomToken.current;
    for (const id of leaves) setLeafFontScale(id, zoom);
    if (appliedZoom.current === zoom) return;
    appliedZoom.current = zoom;
    onResizing(token, true);
    const timer = window.setTimeout(
      () => onResizing(token, false),
      ZOOM_SETTLE_MS,
    );
    return () => window.clearTimeout(timer);
  }, [zoom, leaves, onResizing, setLeafFontScale]);
  useEffect(() => {
    const token = zoomToken.current;
    return () => onResizing(token, false);
  }, [onResizing]);

  const viewRef = useRef(board.view);
  viewRef.current = board.view;

  useEffect(() => {
    const el = canvasRef.current;
    if (!el) return;
    const onWheel = (event: WheelEvent) => {
      const target = event.target as HTMLElement;
      if (target.closest("[data-board-tile-body]")) return;
      event.preventDefault();
      const view = viewRef.current;
      if (event.ctrlKey || event.metaKey) {
        const box = el.getBoundingClientRect();
        const at = { x: event.clientX - box.left, y: event.clientY - box.top };
        update(run, {
          view: zoomAt(view, view.zoom * Math.exp(-event.deltaY * 0.01), at),
        });
      } else {
        update(run, {
          view: { ...view, x: view.x - event.deltaX, y: view.y - event.deltaY },
        });
      }
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [run, update]);

  const zoomBy = (factor: number) =>
    update(run, (b) => ({
      view: zoomAt(b.view, b.view.zoom * factor, {
        x: viewport.w / 2,
        y: viewport.h / 2,
      }),
    }));
  const fit = () =>
    update(run, (b) => ({
      view: fitView(
        leaves.flatMap((id) => (b.rects[id] ? [b.rects[id]] : [])),
        viewport,
      ),
    }));

  const panStart = (event: React.PointerEvent) => {
    if (event.target !== event.currentTarget) return;
    const origin = board.view;
    drag(event, (dx, dy) =>
      update(run, { view: { ...origin, x: origin.x + dx, y: origin.y + dy } }),
    );
  };

  const moveStart = (leafId: number, event: React.PointerEvent) => {
    const origin = board.rects[leafId];
    if (!origin) return;
    const zoom = board.view.zoom;
    drag(event, (dx, dy) =>
      update(run, (b) => ({
        rects: { ...b.rects, [leafId]: moveRect(origin, dx, dy, zoom) },
      })),
    );
  };

  const resizeStart = (
    leafId: number,
    event: React.PointerEvent,
    grid: Grid,
    cell: { width: number; height: number },
  ) => {
    const token = {};
    onResizing(token, true);
    drag(
      event,
      (dx, dy) =>
        update(run, (b) => ({
          grids: { ...b.grids, [leafId]: resizeGrid(grid, dx, dy, cell) },
        })),
      () => onResizing(token, false),
    );
  };

  const setWorldSize = (leafId: number, w: number, h: number) =>
    update(run, (b) => {
      const rect = b.rects[leafId];
      if (!rect || (Math.abs(rect.w - w) < 1 && Math.abs(rect.h - h) < 1)) {
        return {};
      }
      return { rects: { ...b.rects, [leafId]: { ...rect, w, h } } };
    });

  const offCanvas = runTasks.filter(
    (task) => task.pane_id === undefined || !leaves.includes(task.pane_id),
  );
  const dot = Math.max(8, Math.round(24 * board.view.zoom));

  return (
    <div className="flex h-full w-full flex-col bg-background">
      <PlannerBar
        run={run}
        cwd={cwd}
        consoleLeaf={consoleLeaf}
        writeToLeaf={props.writeToLeaf}
        whenLeafReady={props.whenLeafReady}
      />
      <div
        ref={canvasRef}
        onPointerDown={panStart}
        className="relative min-h-0 flex-1 cursor-grab overflow-hidden active:cursor-grabbing"
        style={{
          backgroundImage:
            "radial-gradient(color-mix(in oklab, var(--border) 80%, transparent) 1px, transparent 1px)",
          backgroundSize: `${dot}px ${dot}px`,
          backgroundPosition: `${board.view.x}px ${board.view.y}px`,
        }}
      >
        {leaves.map((leafId) => {
          const rect = board.rects[leafId];
          const grid = board.grids[leafId];
          if (!rect || !grid) return null;
          return (
            <BoardTile
              key={leafId}
              leafId={leafId}
              rect={rect}
              grid={grid}
              view={board.view}
              viewport={viewport}
              focused={leafId === activeLeafId}
              task={taskByLeaf.get(leafId)}
              isConsole={leafId === consoleLeaf}
              supervised={board.supervising}
              cwd={cwd}
              useLeafCell={props.useLeafCell}
              renderLeaf={renderLeaf}
              onMoveStart={moveStart}
              onResizeStart={resizeStart}
              onWorldSize={setWorldSize}
            />
          );
        })}

        {leaves.length === 1 && runTasks.length === 0 ? (
          <div className="pointer-events-none absolute right-6 bottom-14 max-w-xs text-right text-xs text-muted-foreground">
            Describe a job above and press Plan, or add tasks by hand. Each task
            gets its own branch and worktree and appears here as a live
            terminal.
          </div>
        ) : null}

        <OffCanvasTasks tasks={offCanvas} />

        <div className="absolute right-3 bottom-3 flex items-center gap-0.5 rounded-md border border-border/60 bg-card/95 p-0.5 text-xs shadow-sm">
          <ZoomButton
            label="Zoom out"
            icon={MinusSignIcon}
            onClick={() => zoomBy(1 / ZOOM_STEP)}
          />
          <button
            type="button"
            onClick={() =>
              update(run, (b) => ({ view: { ...b.view, zoom: 1 } }))
            }
            className="w-11 rounded px-1 py-0.5 text-center tabular-nums text-muted-foreground hover:bg-accent hover:text-foreground"
            title="Reset zoom"
          >
            {Math.round(board.view.zoom * 100)}%
          </button>
          <ZoomButton
            label="Zoom in"
            icon={PlusSignIcon}
            onClick={() => zoomBy(ZOOM_STEP)}
          />
          <button
            type="button"
            onClick={fit}
            title="Fit all tiles"
            className="rounded px-1.5 py-0.5 text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            Fit
          </button>
        </div>
      </div>
    </div>
  );
}

function ZoomButton({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: typeof PlusSignIcon;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
    >
      <HugeiconsIcon icon={icon} size={13} strokeWidth={1.75} />
    </button>
  );
}

type TileProps = {
  leafId: number;
  rect: Rect;
  grid: Grid;
  view: View;
  viewport: Size;
  focused: boolean;
  task: BoardTask | undefined;
  isConsole: boolean;
  supervised: boolean;
  cwd: string;
  useLeafCell: (leafId: number) => Cell | null;
  renderLeaf: (leafId: number, visible: boolean) => ReactNode;
  onMoveStart: (leafId: number, event: React.PointerEvent) => void;
  onResizeStart: (
    leafId: number,
    event: React.PointerEvent,
    grid: Grid,
    cell: { width: number; height: number },
  ) => void;
  onWorldSize: (leafId: number, w: number, h: number) => void;
};

/** A tile is `cols x rows` cells of its terminal's current font, so its size
 *  follows the font and the text never reflows. */
function BoardTile(props: TileProps) {
  const { leafId, rect, grid, view, viewport, focused, onWorldSize } = props;
  const measured = props.useLeafCell(leafId);
  const cell = measured ?? FALLBACK_CELL;
  const { w: width, h: height } = tileSize(grid, cell);

  useEffect(() => {
    onWorldSize(leafId, width / view.zoom, height / view.zoom);
  }, [leafId, width, height, view.zoom, onWorldSize]);

  const origin = toScreen(rect, view);
  const screen = { x: origin.x, y: origin.y, w: width, h: height };
  return (
    <div
      data-board-tile={leafId}
      className={
        focused
          ? "absolute flex cursor-default flex-col overflow-hidden rounded-lg border border-ring/60 bg-card shadow-md"
          : "absolute flex cursor-default flex-col overflow-hidden rounded-lg border border-border/70 bg-card shadow-sm"
      }
      style={{ left: screen.x, top: screen.y, width, height }}
    >
      <TileHeader
        task={props.task}
        isConsole={props.isConsole}
        supervised={props.supervised}
        cwd={props.cwd}
        onPointerDown={(event) => props.onMoveStart(leafId, event)}
      />
      <div data-board-tile-body className="relative min-h-0 flex-1">
        {props.renderLeaf(leafId, intersects(screen, viewport))}
      </div>
      <div
        aria-hidden
        onPointerDown={(event) =>
          props.onResizeStart(leafId, event, grid, cell)
        }
        className="absolute right-0 bottom-0 z-10 size-3 cursor-nwse-resize"
      />
    </div>
  );
}
