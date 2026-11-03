---
name: terax-orchestra
description: Split a coding job into parallel worker agents with the Terax orchestrator. Use when running inside a Terax terminal (TERAX_TERMINAL is set) and the user asks to parallelize work, delegate tasks to workers or sub-agents, fan out, or orchestrate.
---
<!-- terax-orchestra-skill-v1: managed by Terax, rewritten on update -->

# Terax orchestration

Each worker is another coding agent in its own Terax tab, git worktree and
branch `task/<id>`. Workers run sandboxed: they edit and commit only on their
branch, never push or merge. At most 4 work at once; the rest queue.

Run the CLI as `"$TERAX_CLI"`: every Terax pane sets it, while a `terax` found
on PATH may be another program. Below, `terax` stands for `"$TERAX_CLI"`.

## Workflow

1. From the repository root, once per job:
   `terax orchestrate --check "<fast verification command>"`
   The check (for example `pnpm test --silent` or `cargo test -q`) runs in
   each worktree after the worker finishes. Omit `--check` if there is none.
2. Split the job into independent tasks that touch different files. Write
   one brief per task to a temporary file. A good brief states:
   - the goal and why it matters,
   - the files or area to change and what not to touch,
   - how to verify the result and what done means.
3. Start each task:
   `terax task create --brief <file> --title "<short title>" [--agent claude|codex|gemini] [--model <name>]`
4. Loop until every task is reported:
   `terax task wait` blocks until some task finishes, needs input or fails
   its checks, and prints each one once. It returns at once when nothing is
   in flight.
5. For each reported task, read `terax task result <id>`: the worker's
   `result.md` summary plus the tail of the check log.
   - Needs a fix or answer: `terax task send <id> "<one line instruction>"`,
     then wait again.
   - Done: review with `git diff <base>...task/<id>` from the repository.
   - Hopeless: `terax task stop <id>`.
6. Report to the user which branches are ready. Do not merge task branches
   yourself unless the user asks; worktrees stay until the user removes them.

`terax task list` shows every task. `terax run stop` stops all workers.
Commands fail with `forbidden` inside a worker; only the orchestrator pane
may create and manage tasks.
