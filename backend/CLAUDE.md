# CLAUDE.md

This project follows the `gb-sdlc` Spec-Driven Development framework. The master instructions are shared across all AI coding agents (Claude Code, Antigravity, Cursor) and live in `GEMINI.md`; they are imported below so there is a single source of truth.

@GEMINI.md

## Claude Code Notes

- Read `HANDOFF.md` first at the start of every session and update it before finishing.
- Agent personas (Spectre, Researcher, Planner, Executor, Verifier) are prompt presets in `.gb-sdlc/agents/`; adopt them by reading the file, or run them as subagents via the Agent tool, giving each the preset file path.
- When the user asks to plan an approved spec ("plan out the spec", "plan docs/specs/x.md"), do not plan it in this chat. Run the `/gb-plan` procedure from `.claude/skills/gb-plan/SKILL.md` (use the complex/deep variant only if the spec's `complexity` says so): estimator, parallel planners, then plan auditor. The same applies to "execute the plan" (`/gb-execute`) and "verify the plan" (`/gb-verify`).
- Read active skills listed in `.gb-sdlc/config.yaml` before planning or writing code.
- Add project-specific Claude Code guidance (build/test commands, conventions) below this line.

## Project notes (2026-11-24)

- **Owner**: one person maintains frontend and backend since 2026-11-24. Talk to them in Thai; code, comments, commits and docs stay in English.
- **`/gb-plan`, `/gb-execute`, `/gb-verify`**: these are only entry-point names. The procedures are `.gb-sdlc/workflows/02-planning-battleground.md`, `03-execution-deviation.md` and `04-verification-audit.md`, and the roles they spawn are the presets in `.gb-sdlc/agents/` (`plan-estimator`, `planner`, `battleground-contender`, `plan-auditor`, `executor`, `verifier`), written for Gemini (`GEMINI.md`) and shared with every agent. Thin Claude Code wrappers live in `.claude/skills/gb-plan`, `gb-execute`, `gb-verify` (added 2026-11-24); they point at those workflows and presets and add only project notes. Model tiers per role are in `.gb-sdlc/config.yaml` under `models:`.
- **Commands**: `make help` lists everything. `make test` and `make check` are local. Targets marked `[CLOUD]` change real Cloudflare state: ask the owner first.
- **One register**: a single `HANDOFF.md` and `TODO.md` cover both Workers. New proto-spec IDs continue from `TODO-015`; section 0 of `TODO.md` maps the old per-service IDs.
- **Never commit** `.dev.vars`, secrets, real reader answers, audio or holdout data.
