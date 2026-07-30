# Page & Component Specs

The reference layer: per-page / per-component specs capturing intent, data, auth,
states, tech decisions, and estimates. The **single source of truth** for each screen.

- **Pages** live in [`pages/`](pages/) — use [`_page-template.md`](_page-template.md).
- **Components** live in [`components/`](components/) — use [`_component-template.md`](_component-template.md).
- **[Open questions](OPEN-QUESTIONS.md)** — consolidated, themed; answer once and they propagate back into the specs.
- **[Build planning](../PLANNING.md)** — schedule derived from these specs' estimates.

## How they relate
A **page spec stays thin**: it composes blocks and links out. Any block large
enough to own its own data/states/estimate gets its own component spec under
`components/`, linked from the page's **Sub-component specs** list. Components link
back via their **Used on** header. Small one-off bits stay inline on the page.

> **Pages compose, components own.**

## Conventions
- **Reusable foundations** (inputs, dropdowns, cards, tables, etc.) → one component
  spec each. Map to a library primitive where one fits; build custom where none does.
- **Multi-step flows** (e.g. wizards) → steps fold into the page spec as sections;
  the page owns step flow, branching, and flow-level states (discard / success /
  submitting). Foundations used inside still get their own specs.
- **States table** — every spec ends with one row per state (Default / Loading /
  Empty / Error / …) describing the behavior.
- **Estimate table** — every spec ends with a build estimate broken down by scope;
  these roll up into [PLANNING.md](../PLANNING.md).

## Index

Empty on purpose. GoghMode has three surfaces, each small enough that its
behavior is described in full by [ARCHITECTURE.md](../ARCHITECTURE.md) and read
directly from the code:

| Surface | Where it lives |
|---|---|
| Mac window — toolbar, canvas, status bar | `src/app.rs` |
| Phone / tablet web page | `mobile/index.html` |
| iPad companion — setup screen, drawing screen | `ipad-companion/GoghModeCompanion/ContentView.swift` |

Write a spec here when a surface stops being obvious — the first candidate is the
multi-page notes overview, which is a genuine design problem with states worth
agreeing on before building (see [PLANNING.md](../PLANNING.md) Phase 2).

### Pages
_none yet_

### Components
_none yet_
