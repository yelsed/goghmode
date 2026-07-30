# Documentation

The map of all project documentation. Each doc states its **audience** so you know where to look.

| Doc | Audience | Purpose |
|---|---|---|
| [OVERVIEW.md](OVERVIEW.md) | 👥 Everyone / meetings | Plain-language tour of what the app does, feature by feature. Read the top of each section in a meeting; expand the details for depth. |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 🛠️ Developers | How the system fits together — stack, folder structure, data flow, auth, conventions. Read before contributing. |
| [specs/](specs/README.md) | 🛠️ Developers | Deep per-page / per-component specs: layout, data, states, estimates. The reference layer. |
| [decisions/](decisions/README.md) | 🛠️ Developers | Architecture Decision Records — the *why* behind the big choices, so they aren't re-litigated. |
| [PLANNING.md](PLANNING.md) | 📋 PM / planning | Build schedule and milestones. _(Optional — delete if tracked elsewhere.)_ |
| [specs/OPEN-QUESTIONS.md](specs/OPEN-QUESTIONS.md) | 🛠️ Developers | Resolved decisions + remaining TODOs from the spec phase. |

## How these relate
- **OVERVIEW** = the "what & why", readable. It *summarizes and links* — it never re-explains the specs.
- **ARCHITECTURE** = the "how it's built", for developers.
- **specs/** = the "exact details", the single source of truth for each screen.
- **decisions/** = the "why we chose X".

## Keeping docs alive
- Docs are Markdown, versioned with the code. Update them in the **same PR** as the change.
- A feature isn't "done" until its OVERVIEW status and any affected spec are updated.
- Link, don't duplicate — one source of truth per fact.
