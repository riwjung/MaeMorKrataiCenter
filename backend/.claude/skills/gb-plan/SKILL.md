---
name: gb-plan
description: Plan an APPROVED backend spec with the gb-sdlc factory (estimator, parallel planners with battlegrounds, plan auditor). Use when the owner says "gb-plan", "plan the spec", "plan docs/specs/SPEC-xxx", or wants SPEC-002 or a later spec broken into tasks. Not for writing or changing a spec (that is Spectre, workflow 01) and not for executing tasks (gb-execute).
---

# gb-plan

Entry point for `.gb-sdlc/workflows/02-planning-battleground.md`. That workflow and the role presets in `.gb-sdlc/agents/` are the source of truth; this file only says how to run them from Claude Code. Work inside `backend/`.

## 0. Before anything

1. Read `HANDOFF.md`, then the target spec in `docs/specs/`.
2. **Gate**: the spec front matter must have `status: APPROVED` and `spectre_audit: PASSED`. If not, stop and tell the owner; spec work is workflow 01 with `agents/spectre.md`.
3. Read `.gb-sdlc/config.yaml` (`models`, `token_budget`, `plan_conventions`, `skills`) and the active skills it lists (`.gb-sdlc/skills/<name>/SKILL.md`).
4. Note the spec's `complexity`. `complex` → planners on `opus` / medium effort. `extreme` → `opus` / high, only after the owner confirms. Otherwise `models.planner`.

## 1. Estimate (one agent)

Spawn one Agent with `models.plan_estimator`. Prompt: "Act as `.gb-sdlc/agents/plan-estimator.md` for `docs/specs/<spec>.md`." It writes `docs/plans/<spec-name>-ESTIMATE.md` (plan cut, dependencies, Shared Contract).

**Checkpoint**: show the owner the plan cut in a few lines and wait. The workflow lets the human adjust the cut before planners start.

## 2. Plan (one agent per plan, in parallel)

For each plan in the estimate, spawn a fresh Agent with the planner model from step 0, all in one message. Prompt each with: the preset `.gb-sdlc/agents/planner.md`, the spec path, the estimate path, and the single plan it owns. Planners:
- write `docs/plans/<plan-name>/00_OVERVIEW.md` (with Interface Ledger, Cross-Plan Assumptions, Proto-Spec Candidates), `01_<TASK>.md` … and `NN_DOCUMENTATION.md`;
- run battlegrounds (`agents/battleground-contender.md`, `models.battleground`) where several approaches exist, saved under `docs/battlegrounds/<plan-name>/<task-name>/debate.md`;
- never write `HANDOFF.md` or `TODO.md`.

## 3. Audit (one agent)

Spawn one Agent with `models.plan_auditor`: "Act as `.gb-sdlc/agents/plan-auditor.md` for `<spec-name>`." It fixes task files in place and writes `docs/plans/<spec-name>-PLAN-AUDIT.md` with the global execution order and a verdict, merges proto-spec candidates into `TODO.md` (next free ID is in its header), and updates `HANDOFF.md`.

## 4. Report

- `ALIGNED`: tell the owner the number of plans and tasks and that `gb-execute` is next.
- `NEEDS HUMAN`: list the open questions; do not execute.

## Rules

- Every spawned agent gets an explicit `model` (and effort in its prompt); never let it inherit the session model.
- Keep each agent under `token_budget.target`; at `over`, it stops and writes state to `HANDOFF.md`.
- Talk to the owner in Thai; all files stay in English.
