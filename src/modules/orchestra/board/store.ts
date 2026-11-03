import { create } from "zustand";
import type { Grid, Rect, View } from "./lib/canvas";
import type { Draft, Planner } from "./lib/drafts";

export type BoardState = {
  job: string;
  planner: Planner;
  check: string;
  drafts: Draft[];
  planning: boolean;
  /** A supervisor agent should answer the workers and merge their work. */
  supervise: boolean;
  supervising: boolean;
  rects: Record<number, Rect>;
  grids: Record<number, Grid>;
  view: View;
};

const EMPTY: BoardState = {
  job: "",
  planner: "claude",
  check: "",
  drafts: [],
  planning: false,
  supervise: true,
  supervising: false,
  rects: {},
  grids: {},
  view: { x: 32, y: 32, zoom: 1 },
};

type Store = {
  boards: Record<string, BoardState>;
  update: (
    run: string,
    change: Partial<BoardState> | ((board: BoardState) => Partial<BoardState>),
  ) => void;
};

// Board layout lives for the app session only; board tabs are not restored.
export const useBoardStore = create<Store>((set) => ({
  boards: {},
  update: (run, change) =>
    set((state) => {
      const board = state.boards[run] ?? EMPTY;
      const patch = typeof change === "function" ? change(board) : change;
      if (Object.keys(patch).length === 0) return state;
      return { boards: { ...state.boards, [run]: { ...board, ...patch } } };
    }),
}));

export function useBoard(run: string): BoardState {
  return useBoardStore((state) => state.boards[run] ?? EMPTY);
}
