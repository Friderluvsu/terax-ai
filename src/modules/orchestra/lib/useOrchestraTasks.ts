import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { BoardTask } from "./board";

export function useOrchestraTasks(): BoardTask[] {
  const [tasks, setTasks] = useState<BoardTask[]>([]);
  useEffect(() => {
    let disposed = false;
    let generation = 0;
    const refresh = () => {
      const current = ++generation;
      invoke<BoardTask[]>("orchestra_tasks")
        .then((next) => {
          if (!disposed && current === generation) setTasks(next);
        })
        .catch((error) => console.warn("[terax] orchestra tasks:", error));
    };
    const unlisten = listen("terax:orchestra-changed", refresh);
    refresh();
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, []);
  return tasks;
}
