# Documentation

The map of all project documentation. Each doc states its **audience** so you know where to look.

| Doc | Audience | Purpose |
|---|---|---|
| [OVERVIEW.md](OVERVIEW.md) | 👥 Everyone / meetings | Plain-language tour of what the app does, feature by feature. Read the top of each section in a meeting; expand the details for depth. |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 🛠️ Developers | How the system fits together — stack, folder structure, data flow, auth, conventions. Read before contributing. |
| [specs/](specs/README.md) | 🛠️ Developers | Deep per-page / per-component specs: layout, data, states, estimates. The reference layer. |
| [decisions/](decisions/README.md) | 🛠️ Developers | Architecture Decision Records — the *why* behind the big choices, so they aren't re-litigated. |
| [PLANNING.md](PLANNING.md) | 📋 PM / planning | What's next and what gates what. Ordered, not scheduled — no dates, no estimates. |
| [specs/OPEN-QUESTIONS.md](specs/OPEN-QUESTIONS.md) | 🛠️ Developers | Resolved decisions + the questions still to answer. |

## Background / source documents

These came first and are kept for their reasoning and detail. **The docs above are
the current map** — where the two disagree, the map wins.

| Doc | What it is |
|---|---|
| [ai-field-notebook-vision.md](ai-field-notebook-vision.md) | The vision: the friction being removed, device roles, capture modes, non-goals. Still authoritative on *why*. |
| [mobile-companion-roadmap.md](mobile-companion-roadmap.md) | The mobile bridge roadmap. Its MVP is fully shipped; its bridge contract is now an [ADR](decisions/0001-drawings-latest-as-the-agent-contract.md). |
| [pencilkit-deployment-todo.md](pencilkit-deployment-todo.md) | The Apple signing log. The TestFlight half is a live runbook; the cable-install half is superseded. |
| [superpowers/plans/](superpowers/plans/) | The executed implementation plan for the iPad companion. Historical — the code diverged from it deliberately in two places. |
| [`later.md`](../later.md) (repository root) | The deferred-work register from the first round of iPad feedback. Feeds [PLANNING.md](PLANNING.md) directly. |
| [companion-multi-host-planning-prompt.md](companion-multi-host-planning-prompt.md) | The brief for the multi-host work: requirements, security constraints, and the analysis it asked for. |
| [companion-multi-host-plan.md](companion-multi-host-plan.md) | The answer to that brief. Threat model, design comparison, pairing protocol, cross-platform split, Omarchy deployment, migration, and ordered phases. Decision recorded as [ADR-0006](decisions/0006-paired-devices-over-shared-url-token.md). |

## How these relate
- **OVERVIEW** = the "what & why", readable. It *summarizes and links* — it never re-explains the specs.
- **ARCHITECTURE** = the "how it's built", for developers.
- **specs/** = the "exact details", the single source of truth for each screen.
- **decisions/** = the "why we chose X".

## Keeping docs alive
- Docs are Markdown, versioned with the code. Update them in the **same PR** as the change.
- A feature isn't "done" until its OVERVIEW status and any affected spec are updated.
- Link, don't duplicate — one source of truth per fact.
