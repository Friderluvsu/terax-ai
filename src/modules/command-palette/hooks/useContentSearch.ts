import { currentWorkspaceEnv, fsFor } from "@/modules/workspace";
import { useCallback } from "react";
import { type AsyncQueryState, useAsyncQuery } from "./useAsyncQuery";

export const CONTENT_SEARCH_MIN_QUERY = 2;
const LIMIT = 80;
const DEBOUNCE_MS = 140;

export type ContentHit = {
  path: string;
  rel: string;
  line: number;
  text: string;
};

export function useContentSearch(
  root: string | null,
  term: string,
  enabled: boolean,
): AsyncQueryState<ContentHit> {
  const run = useCallback(
    async (q: string): Promise<ContentHit[]> => {
      if (!root) return [];
      const res = await fsFor(currentWorkspaceEnv()).grepInteractive({
        pattern: q,
        root,
        maxResults: LIMIT,
      });
      return res.hits;
    },
    [root],
  );

  return useAsyncQuery({
    enabled: enabled && !!root,
    term,
    minLength: CONTENT_SEARCH_MIN_QUERY,
    debounceMs: DEBOUNCE_MS,
    run,
  });
}
