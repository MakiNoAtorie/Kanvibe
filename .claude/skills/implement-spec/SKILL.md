---
name: implement-spec
description: "Implement the result of /to-spec and /to-tickets in code."
disable-model-invocation: true
---

You have been provided a spec. This spec should have tickets associated with it, describing how to implement the spec.

The issue tracker should have been provided to you. If not, tell the user to run `/setup-matt-pocock-skills`.

The goal is the entire spec implemented on a single **integration branch**, with every ticket resolved the way the issue tracker closes work.

The tickets are not a list of steps. They are a **task graph** with blocking relationships between them. This means there is always a **frontier** of tickets which are ready to be grabbed.

Communication to and from subagents should be sparse. Communicate primarily through **context pointers**: to the spec, tickets, research notes, and previous commits. Don't duplicate information already available via pointers.

**Implementer subagents** should be run in the background where possible for maximum concurrency.

## Steps

1. Read the spec and tickets to understand the task graph.

2. (optional) Use an **exploration subagent** to conduct any exploration required by the tickets - relevant codebase files or external documentation. Ensure the exploration subagent can save files - it should save its markdown notes in a directory outside the repo, accessible by all future subagents. This lets **implementer subagents** focus on implementation rather than exploration.

3. Create the integration branch. If the issue tracker closes work through PRs, or the user asks for one, open a draft PR after the first merge in step 5 (a branch with no commits ahead of main can't open one), marked as closing the spec and tickets.

4. Start one **implementer subagent** per ticket with `subagent_type: "implementer"`, each in its own worktree on its own branch, in the background. Its rules (base on the integration branch, `tdd`, filtered output, local checks only, no CI polling, report format) live in `.claude/agents/implementer.md`; don't repeat them. The prompt carries only the ticket, branch, integration branch, scratchpad path, files in flight and any excerpts. Before each launch, choose the model and say why in one line: `model: "opus"` for concurrency, state/replay/serialization, rendering, security-sensitive parsing, cross-module refactors or unexplained failures; `model: "sonnet"` for well-specified work that follows existing patterns (UI, CLI wiring, config, docs, small fixes). Don't read implementer transcripts; wait for their reports.

5. Once an **implementer subagent** completes, merge its work to the integration branch. Fast-forward merges go directly; use a **merger subagent** (`model: "sonnet"`) only for real conflicts. Check CI once at merge. If it's red, send the failing job and its error lines (not the full log) back to the same implementer with SendMessage; don't start a new one. Fold the report's convention bullets into `docs/CONVENTIONS.md`.

6. If this changes the **frontier** of available tickets, kick off more **implementer subagents** to work on the new tickets. This allows for maximum concurrency.

7. Once all tickets are complete, call the Skill tool with `code-review` on the integration branch. Fix all issues raised by the code review in a single **implementer subagent**.

8. If a draft PR exists, mark it ready for review. Otherwise, resolve each ticket the way the issue tracker closes work, and report the integration branch.

9. Clean up all **implementer subagent** worktrees.
