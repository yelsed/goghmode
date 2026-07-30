# {{Page Name}}

> Route: `/{{path}}` · Design: {{link}} · Status: draft / in progress / done

## Goal & user
Who uses this page and what do they need to accomplish?

## Layout
Structure of the page (regions, columns, key blocks, spacing).
- ...

## Components
The blocks this page is built from. Name a library primitive where one fits; flag
where none does and a custom build is needed.
- ...

### Sub-component specs
Larger blocks that own their own data/states get their own spec file. Link them
here; this page only composes them.
- [ ] [{{Component}}](../components/{{component}}.md)

## Design tokens
Colors, type scale, fonts, spacing this page relies on.
- ...

## Tech used
Decisions on libraries/patterns for this page (and why, if non-obvious).
- Data fetching / state: ...
- Forms / validation: ...
- Other: ...

## Auth & permissions
- Required role(s) / policy:
- Route middleware / guard:
- Login requirements:

## Data
- Collections / tables / endpoints & fields:
- Queries / mutations:
- Types:

## Client state
Server state belongs under **Data**. List here only what the server doesn't own.
- Store / local component state of note:

## Routes & redirects
- Route config:
- Redirects on success / failure:

## States
One row per state. Link a design frame where there's a dedicated design; otherwise describe behavior.

| State | Design | Behavior |
| --- | --- | --- |
| Default | _link_ | |
| Loading | _link_ | |
| Empty | _link_ | |
| Error | _link_ | |
| Unauthorized | _link_ | |

## Estimate
Rough build time, broken down by scope. Ranges, not promises.

| Scope | Estimate |
| --- | --- |
| Layout & markup | |
| Components | |
| Data & state wiring | |
| Auth / permissions | |
| States (loading/empty/error) | |
| Polish & responsive | |
| **Total** | |

## Tasks
- [ ] ...

## Open questions
- ...
