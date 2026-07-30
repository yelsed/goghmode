# Architecture

> **Audience:** developers. How the system fits together — read before
> contributing. Update this when a **structural pattern** changes (not for
> per-screen detail — that lives in [specs/](specs/README.md)).

## Stack
The languages, frameworks, and key libraries, each with a one-line "why it's here".
- ...

## System diagram
A diagram (ASCII / Mermaid) of the major pieces and how they talk — client,
server, data store, external services, auth.

## Folder structure
Where things live and what belongs in each directory. Keep it to the dirs a
contributor needs to navigate.
- ...

## Shared component / module kit
The reusable building blocks that already exist, so people **reuse before
building**. List each primitive with a one-line purpose. Update this index when
you add or extract a shared primitive.
- ...

## Data flow (reading & writing)
How data moves through the app — fetching, caching, mutations, invalidation. Name
the pattern and where the rules live.
- ...

## Loading / empty / error convention
The single agreed way every screen handles these states, so they're consistent.
- ...

## Auth
How authentication & authorization work — sessions/tokens, roles/permissions,
route protection.
- ...

## State management — the rule
What owns server state vs. client state, and the one rule contributors follow.
- ...

## Conventions
Naming, file layout, formatting, lint rules, and any non-obvious idioms a
contributor must match.
- ...

## Testing
How the project is tested (unit / integration / e2e), how to run them, and what
"done" requires.
- ...

## Related
- [OVERVIEW.md](OVERVIEW.md) · [specs/](specs/README.md) · [decisions/](decisions/README.md)
