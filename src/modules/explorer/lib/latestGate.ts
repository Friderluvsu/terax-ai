export type LatestGate = {
  begin: (key: string) => number;
  isLatest: (key: string, ticket: number) => boolean;
  clear: () => void;
};

// Remote listings can resolve out of order; only the newest request per key
// may apply its result.
export function createLatestGate(): LatestGate {
  const latest = new Map<string, number>();
  let counter = 0;
  return {
    begin(key) {
      counter += 1;
      latest.set(key, counter);
      return counter;
    },
    isLatest: (key, ticket) => latest.get(key) === ticket,
    clear: () => latest.clear(),
  };
}
