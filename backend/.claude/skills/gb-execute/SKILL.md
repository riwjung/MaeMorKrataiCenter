---
name: gb-execute
description: Execute an audited gb-sdlc backend plan task by task, one fresh executor agent per task, with deviation escalation. Use when the owner says "gb-execute", "execute the plan", or "run the tasks" after gb-plan produced an ALIGNED plan audit. Not for planning (gb-plan) or final verification (gb-verify).
---

# gb-execute

Entry point for `.gb-sdlc/workflows/03-execution-deviation.md`, using the preset `.gb-sdlc/agents/executor.md`. Work inside `backend/`.

## 0. Before anything

1. Read `HANDOFF.md`.
2. **Gate**: `docs/plans/<spec-name>-PLAN-AUDIT.md` exists with verdict `ALIGNED`. If not, stop and point the owner to `gb-plan`.
3. Take the global execution order (plan, task) from the audit file. Resume at the first task whose checkbox in its plan's `00_OVERVIEW.md` is not ticked.

## 1. One task = one fresh agent

For the next task, spawn a new Agent (never reuse one across tasks):
- model and effort: the task file's `model` / `effort`, else `models.executor`. **Never opus.**
- prompt: "Act as `.gb-sdlc/agents/executor.md`. Read `docs/plans/<plan>/00_OVERVIEW.md` first, then execute only `docs/plans/<plan>/<NN_TASK>.md`: harness, tests, red (must fail as stated), implementation, green, standards check, then the task's verification commands. No improvising, no in-code TODOs."

Run tasks one at a time in audit order; only parallelize tasks the audit marks independent.

## 2. After each task

1. Check the agent's report and re-run the task's verification commands yourself.
2. Tick the task in `00_OVERVIEW.md`; update `HANDOFF.md` (What We Have in Current, next task).
3. New non-blocking ideas go to `TODO.md` as proto-specs (next free ID).

## 3. Deviation (a command fails, or existing code conflicts with the task)

Stop. Do not guess and do not switch to a stronger executor model. Spawn a resolution agent (`models.planner`) with the spec, `docs/decisions/`, `00_OVERVIEW.md` and the failing task. If several fixes exist, run a task-level battleground in `docs/battlegrounds/<plan>/<task>/`. If the task is under-specified, the fix is to split or rewrite the task, then re-run it with a fresh executor. Tell the owner what happened.

## 4. Finish

Run `NN_DOCUMENTATION.md` with its own fresh executor. When every task in every plan is ticked, run `make test` and `make check` once, then hand off to `gb-verify`.

## Rules

- Talk to the owner in Thai; files stay in English.
- Never deploy or run `[CLOUD]` make targets as part of execution.
