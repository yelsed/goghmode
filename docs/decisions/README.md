# Architecture Decision Records (ADRs)

> **Audience:** developers. Short records of significant technical decisions — the
> *why*, so they aren't re-litigated later.

Each ADR captures: **context** (the situation), **decision** (what we chose),
**consequences** (the trade-offs). Use [`_template.md`](_template.md) for new ones.
Number them sequentially; **never delete — supersede** with a new ADR and flip the
old one's status.

## When to add one
Any non-obvious technical choice: a library, a data model, an auth strategy, a
pattern you'd otherwise have to explain twice. If someone might ask "why is it done
this way?", write the ADR.

## Index
| # | Decision | Status |
|---|---|---|
| [0001](0001-drawings-latest-as-the-agent-contract.md) | `drawings/latest.*` is the agent contract | Accepted |
| [0002](0002-token-in-path-lan-pairing.md) | The pairing token is the URL path | Accepted |
| [0003](0003-vector-strokes-over-png-upload.md) | Clients upload vector strokes, not rendered images | Accepted |
| [0004](0004-no-http-framework.md) | Hand-written HTTP instead of a framework | Accepted |
| [0005](0005-native-swiftui-over-flutter.md) | Native SwiftUI and PencilKit for the iPad companion | Accepted |
| [0006](0006-paired-devices-over-shared-url-token.md) | Paired devices with per-device secrets, not one shared URL token | Accepted |

The first five were written up on 25 July 2026 from the code and the background
documents. The date on each record is when the decision was *made*, not when it was
written down.
