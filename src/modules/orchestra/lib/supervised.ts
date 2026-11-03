// Board runs that have a supervisor agent answering their workers. Kept
// outside the lazy board chunk so notification routing can read it.
const runs = new Set<string>();

export function markSupervised(run: string): void {
  runs.add(run);
}

export function isSupervised(run: string | undefined): boolean {
  return run !== undefined && runs.has(run);
}
