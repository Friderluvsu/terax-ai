export const meta = {
  name: 'terax-wave',
  description: 'Execute Terax fork-plan task cards: implement on task branches (worktree or main checkout), optional review, one fix round',
  phases: [
    { title: 'Implement', detail: 'one executor per task card, model per card' },
    { title: 'Review', detail: 'only for cards with a reviewer' },
    { title: 'Fix', detail: 'one round for critical/major findings' },
  ],
}

const REPO = '/Users/frider/Documents/iamcoder/terax_claude'
const TRAILER = 'Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>'

const IMPL_SCHEMA = {
  type: 'object',
  properties: {
    status: { type: 'string', enum: ['done', 'blocked'] },
    branch: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    summary: { type: 'string' },
    files: { type: 'array', items: { type: 'string' } },
    gates: { type: 'string', description: 'exact results: lint errors/warnings, tsc, vitest counts, clippy, cargo test counts' },
    deviations: { type: 'string', description: 'anything done differently from the card and why, or empty' },
    manualChecks: { type: 'array', items: { type: 'string' } },
    risks: { type: 'array', items: { type: 'string' } },
  },
  required: ['status', 'branch', 'commits', 'summary', 'files', 'gates', 'deviations', 'manualChecks', 'risks'],
}

const REVIEW_SCHEMA = {
  type: 'object',
  properties: {
    verdict: { type: 'string', enum: ['approve', 'changes'] },
    findings: { type: 'array', items: { type: 'object', properties: {
      severity: { type: 'string', enum: ['critical', 'major', 'minor'] },
      file: { type: 'string' }, problem: { type: 'string' }, fix: { type: 'string' } },
      required: ['severity', 'file', 'problem', 'fix'] } },
  },
  required: ['verdict', 'findings'],
}

const where = (t) => t.isolation === 'worktree'
  ? `You run in a fresh git worktree of the repo (your current directory). Work only inside it. Run \`pnpm install --frozen-lockfile --offline\` once before any pnpm command (fall back to without --offline only if it fails).`
  : `You work directly in the main checkout ${REPO}. No other agent uses git in this checkout while you run. Do not run pnpm install (already installed). Do not start the GUI app (pnpm tauri dev / fork:dev); the integrator does manual checks.`

const cargo = (t) => t.slot
  ? `For EVERY cargo command export CARGO_TARGET_DIR=${t.slot} CARGO_INCREMENTAL=0 (disk is nearly full: never create other target dirs, never run cargo build --release).`
  : t.rust ? `cargo uses the default src-tauri/target. Disk is nearly full: never run cargo build --release.` : `This task needs no cargo commands; do not run cargo.`

const implPrompt = (t) => `You implement task ${t.id} of the Terax fork plan. ${where(t)}
Setup:
1. \`source scripts/fork/agent-env.sh\` at the start of every shell command that uses node, pnpm or cargo.
2. \`git switch -c task/${t.id} fork/main\` (fork/main is the integration branch).
3. ${cargo(t)}
Your task card: run \`sed -n '/^#### ${t.id} /,/^#### /p' fork-plan/PLAN.md\` and implement exactly that card. Section 3 of fork-plan/PLAN.md has the gates; the epic header above the card names the scouting report in fork-plan/scouts/ with file:line evidence. Read only what you need; verify line numbers in the code before editing.
${t.note ? `Integrator note: ${t.note}\n` : ''}Rules: follow TERAX.md and FORK.md (already in your context). Stay inside the card's scope. No em-dash, no emojis, minimal comments, '@/' imports. Format only files you touched: \`rustfmt <file>\` and \`pnpm exec biome format --write <file>\`; NEVER run \`cargo fmt\`, \`npx prettier\` or a formatter on directories. Before committing run \`git status --short\` and commit only files your card needs (no backup files such as *-E or *.orig, no unrelated formatting).
Finish:
- Run the gates the card lists. Lint must stay at 0 errors and at most 90 warnings. If a gate fails because of a pre-existing issue unrelated to your change, say so explicitly in gates.
- Commit everything in one commit titled "${t.id}: <short title>" whose message ends with the line: ${TRAILER}
- Then release the branch: ${t.isolation === 'worktree' ? '`git switch --detach`' : '`git switch fork/main` and confirm `git status --short` is clean apart from untracked fork-plan/raw and fork-plan/research'}.
- If you cannot complete the card, commit nothing broken, set status "blocked" and explain.
Return the structured result.`

const reviewPrompt = (t, impl) => `Review task ${t.id} of the Terax fork plan (repo ${REPO}). You are READ-ONLY: never edit files, never switch branches in the main checkout; use \`git -C ${REPO} diff fork/main...task/${t.id}\`, \`git -C ${REPO} show task/${t.id}:<path>\` and reading files.
Card: \`sed -n '/^#### ${t.id} /,/^#### /p' ${REPO}/fork-plan/PLAN.md\`.
Executor summary: ${JSON.stringify({ summary: impl.summary, deviations: impl.deviations, gates: impl.gates })}
Look for: unmet acceptance criteria, security holes (argument injection, path traversal, symlink escape, fail-open, IPC exposure), data loss, races, process leaks, eager imports of heavy code, missing invariant tests, broken conventions. Report only real problems with a concrete fix; minor nits as minor. verdict "approve" if nothing critical or major.`

const fixPrompt = (t, findings) => `You fix review findings for task ${t.id} of the Terax fork plan. ${where(t)}
Setup: \`source scripts/fork/agent-env.sh\`; \`git switch task/${t.id}\`; ${cargo(t)}
Card for context: \`sed -n '/^#### ${t.id} /,/^#### /p' fork-plan/PLAN.md\`.
Fix exactly these findings (critical/major), nothing else:
${JSON.stringify(findings, null, 1)}
Run the card's gates, commit "${t.id}: address review" ending with the line ${TRAILER}, then release the branch (${t.isolation === 'worktree' ? '`git switch --detach`' : '`git switch fork/main`'}). Return the structured result.`

const run = (t) => {
  const opts = (phase, label) => ({ label, phase, model: t.model, effort: t.effort, schema: IMPL_SCHEMA, ...(t.isolation === 'worktree' ? { isolation: 'worktree' } : {}) })
  return agent(implPrompt(t), opts('Implement', `impl:${t.id}`))
}

const results = await pipeline(
  args.tasks,
  (t) => run(t),
  (impl, t) => {
    if (!impl || impl.status !== 'done' || !t.review) return { impl, review: null }
    return agent(reviewPrompt(t, impl), { label: `review:${t.id}`, phase: 'Review', model: t.review, effort: 'high', schema: REVIEW_SCHEMA })
      .then((review) => ({ impl, review }))
  },
  (prev, t) => {
    const blocking = (prev.review?.findings ?? []).filter((f) => f.severity !== 'minor')
    if (!blocking.length) return { ...prev, fix: null }
    return agent(fixPrompt(t, blocking), { label: `fix:${t.id}`, phase: 'Fix', model: t.model, effort: t.effort, schema: IMPL_SCHEMA, ...(t.isolation === 'worktree' ? { isolation: 'worktree' } : {}) })
      .then((fix) => ({ ...prev, fix }))
  },
)

return args.tasks.map((t, i) => ({ id: t.id, ...(results[i] ?? { impl: null }) }))
