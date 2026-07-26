# GoghMode Multi-Host Companion Planning Prompt

Copy/paste the prompt below into Sol or another planning LLM.

---

You are a senior systems architect and security engineer. Create an implementation plan for extending the GoghMode project.

Do not write code yet. Produce a concrete, technically validated plan that another developer or coding agent can execute.

## Project context

GoghMode is a local sketchpad for terminal AI workflows.

Current architecture:

- The desktop application is written in Rust using eframe/egui.
- It currently builds on macOS and also passes `cargo check` on Linux.
- A local HTTP server runs inside the desktop application.
- The server currently binds to `0.0.0.0:8787`, with an ephemeral-port fallback.
- It serves a browser drawing companion.
- The native iPad companion also connects to the server.
- The server URL currently looks like:

  `http://<LAN-IP>:<PORT>/<persistent-random-token>/`

- The token is persisted at `~/.goghmode/mobile-token`.
- The browser and iPad companion POST a `DrawingSnapshot` to `/<token>/save`.
- The desktop host validates the snapshot and writes `latest.json`, `latest.svg`, and `latest.png`.
- All writes go through one export pipeline.
- There is no cloud backend, account system, public tunnel, or external relay.
- The current companion model assumes one active desktop host per URL.
- The current app UI and documentation use Mac-specific wording such as “Mac”, “Send to Mac”, and “Mac scratch”.
- The current folder-reveal action uses the macOS `open` command and needs Linux support through `xdg-open`.
- The target Linux environment is Arch Linux running Omarchy/Hyprland.
- The Rust test suite currently passes.

Relevant existing files include:

- `src/main.rs`
- `src/app.rs`
- `src/mobile_server.rs`
- `src/app_install.rs`
- `src/drawing.rs`
- `src/export.rs`
- `mobile/index.html`
- `ipad-companion/GoghModeCompanion/GoghModeClient.swift`
- `docs/ARCHITECTURE.md`
- `docs/mobile-companion-roadmap.md`
- `docs/decisions/0002-token-in-path-lan-pairing.md`
- `README.md`

## User requirements

Safety is the highest priority.

I want the companion app to support multiple GoghMode hosts:

1. The user must be able to add a new host in the companion app.
2. The companion app must be able to connect to two or more hosts.
3. The user is considering whether one URL could connect the app to two hosts.
4. The system must support both macOS and Linux hosts dynamically.
5. The design must not contain Mac-specific assumptions.
6. The existing local drawing workflow should remain intact.
7. The system should not silently send drawings to the wrong host.
8. Host identity, host selection, pairing, and active destination must be visible and understandable to the user.

## Important feasibility question

Analyze this carefully:

“A single URL connecting one companion app to two hosts” can mean several different things:

- One URL that bootstraps a host configuration into the app.
- One URL representing a saved host profile.
- One URL that lets the user choose between multiple hosts.
- One URL that causes every drawing to upload to multiple hosts.
- One URL backed by a relay or central service that fans out uploads.

Explain which interpretations are possible without a backend and which require a relay or additional protocol.

Do not assume that “one URL to two hosts” is technically possible just because it is requested. Identify the safest design that satisfies the intended user experience without creating accidental fan-out or ambiguous destination behavior.

## Security requirements

Treat these as hard requirements unless you explain why a change is necessary:

- No public cloud service.
- No public tunnel.
- No router port forwarding.
- No unauthenticated local server.
- No silent multi-host broadcast.
- No upload to a host that has not been explicitly paired.
- No trusting a host solely because it is on the same LAN.
- Do not expose drawing contents unnecessarily.
- Prevent stale host URLs from silently targeting the wrong machine.
- Make host replacement, token rotation, and revocation possible.
- Consider malicious or compromised devices on the same Wi-Fi.
- Consider hostile or untrusted Wi-Fi.
- Consider replayed URLs.
- Consider a leaked URL.
- Consider port changes.
- Consider two hosts having similar IP addresses or hostnames.
- Consider a user running several GoghMode instances.
- Consider whether plain HTTP is acceptable on a trusted LAN.
- Compare HTTPS with self-signed certificates, local CA certificates, or another pairing mechanism.
- Explain the tradeoff between usability and strong authentication.
- Explain how the companion verifies that the selected host is the intended host.

## Platform requirements

The design must work for:

### macOS host

- Native GoghMode desktop app.
- macOS application bundle may remain macOS-specific.
- Do not leak macOS assumptions into the shared host protocol.

### Linux host

- Arch Linux / Omarchy.
- Wayland/Hyprland.
- Native Rust GUI.
- Linux-friendly launch flow.
- `xdg-open` instead of hardcoded `open`.
- Optional `.desktop` launcher.
- No modifications to Omarchy-managed files under `~/.local/share/omarchy/`.
- Avoid requiring a background daemon in the first version unless security or reliability requires it.
- Consider firewall configuration and local-network interface selection.
- Consider multiple network interfaces, VPNs, Wi-Fi changes, and changing LAN IP addresses.

## Required analysis

### 1. Current-state assessment

Identify what the existing architecture already supports and what it does not support.

Explicitly distinguish:

- Browser companion behavior.
- Native iPad companion behavior.
- Desktop host behavior.
- Pairing and authentication.
- Host persistence.
- Host selection.
- Multi-host uploads.
- macOS-specific code.
- Linux-specific gaps.

### 2. Threat model

Define:

- Assets.
- Trust boundaries.
- Attacker capabilities.
- Security goals.
- Non-goals.
- Failure consequences.

Include at least these attacker scenarios:

- Another device on the same Wi-Fi.
- A guest Wi-Fi user.
- A leaked or screenshotted pairing URL.
- A stale URL after the host IP or port changes.
- A malicious host pretending to be GoghMode.
- A compromised companion device.
- A user accidentally selecting the wrong host.
- A host that is offline or has been replaced.

### 3. Candidate designs

Compare at least four designs:

1. Current secret URL over HTTP.
2. Host profiles with explicit pairing and visible host selection.
3. QR-based pairing with cryptographic host identity.
4. A relay or central fan-out service.

For each design, evaluate:

- Security.
- Privacy.
- Complexity.
- Offline/local-only operation.
- macOS/Linux compatibility.
- User experience.
- Multi-host support.
- Host revocation.
- Recovery from changed IP addresses.
- Migration cost from the current implementation.

### 4. Recommended design

Choose one design. Do not merely list options.

The recommended design should preferably:

- Keep the architecture local-first.
- Allow multiple saved hosts in the companion app.
- Use one pairing link per host, or clearly explain a secure alternative.
- Require explicit host selection before upload.
- Make the active destination obvious.
- Avoid accidental broadcasting.
- Support optional “send to selected hosts” only as an explicit user action if that is safe.
- Give each host a stable identity separate from its IP address.
- Handle IP and port changes without silently changing host identity.
- Work with both macOS and Linux hosts.
- Permit secure migration from current token URLs.

Explain whether the safest interpretation is:

- One URL per host.
- One URL that imports a host list.
- One app containing multiple saved hosts.
- Explicitly selected multi-host broadcast.
- Or another design.

### 5. Protocol and data model

Propose the protocol changes in detail.

Include:

- Host discovery or manual host entry.
- Pairing request and approval flow.
- Host identity.
- Companion identity, if needed.
- Credentials or keys.
- Token rotation.
- Revocation.
- Host display name.
- Platform metadata.
- Protocol version.
- Capabilities.
- Current IP/port.
- Last-seen state.
- Offline state.
- Certificate or public-key fingerprint, if applicable.
- How the companion distinguishes two GoghMode hosts.
- How a pairing URL is structured.
- Whether pairing URLs expire.
- How a host proves possession of its identity.
- How upload requests are authenticated.
- Replay protection.
- Error responses.
- Migration compatibility with existing `/token/save` endpoints.

Do not invent cryptography casually. If public-key authentication is recommended, specify appropriate standard primitives and explain how keys are generated and stored on macOS, iOS/iPadOS, and Linux.

### 6. Multi-host semantics

Define the exact behavior for:

- One saved host.
- Two saved hosts.
- Selecting one active host.
- Sending to multiple hosts.
- A host being offline.
- One host accepting an upload and another rejecting it.
- Partial success.
- Retry behavior.
- Duplicate uploads.
- Ordering.
- User confirmation.
- Whether “send to all” should exist at all.

The default behavior must be conservative and must not broadcast silently.

### 7. Dynamic macOS/Linux host support

Explain how to remove platform assumptions from:

- Host names and labels.
- Status messages.
- Page IDs such as `mac-scratch`.
- Documentation.
- Folder opening.
- App installation.
- Launching.
- Network interface selection.
- Output paths.
- Claude Code skill instructions.

Specify which parts belong in shared Rust code and which parts must remain platform-specific.

### 8. Omarchy deployment

Provide an implementation and deployment plan for Omarchy that covers:

- Building with Cargo.
- Running the host application.
- Optional user-level `.desktop` launcher.
- Optional Hyprland autostart, including why it should or should not be enabled.
- Firewall requirements.
- Network-interface behavior.
- VPN and Wi-Fi changes.
- Safe storage locations.
- Avoiding edits to Omarchy-managed source files.
- Logging and diagnostics.
- How to stop the host and revoke access.
- How to recover from a leaked pairing credential.

### 9. Migration plan

Create a backward-compatible migration plan from the current system:

- Existing browser URLs.
- Existing iPad companion endpoints.
- Existing persistent token files.
- Existing saved drawings.
- Existing schema version 1 and 2 snapshots.
- Existing Claude Code skill behavior.
- Existing macOS users.
- New Linux users.

Specify whether old URLs remain valid, how long they remain valid, and how users explicitly upgrade.

### 10. Testing and verification

Provide a test plan covering:

- Unit tests.
- Protocol tests.
- Pairing tests.
- Authentication tests.
- Replay tests.
- Revocation tests.
- Multiple-host tests.
- Offline-host tests.
- Partial-success tests.
- IP address changes.
- Port changes.
- Wrong-host detection.
- Leaked URL behavior.
- Linux smoke tests.
- macOS smoke tests.
- Browser companion tests.
- Native iPad companion tests.
- Firewall and network-interface tests.
- GUI tests where practical.

Include concrete end-to-end scenarios, not only test categories.

## Required output format

Return the plan in this structure:

# Executive recommendation

State the chosen design in a few paragraphs.

# Feasibility of one URL for two hosts

Explain precisely what is and is not possible.

# Current architecture assessment

Describe the current implementation and gaps.

# Threat model

Use a table with assets, boundaries, attackers, and consequences.

# Design alternatives

Use a comparison table.

# Recommended architecture

Include a Mermaid diagram.

# Pairing and authentication protocol

Describe the complete flow step by step.

# Multi-host behavior

Define exact UX and failure semantics.

# Cross-platform architecture

Separate shared code from macOS-specific and Linux-specific code.

# Omarchy deployment plan

Give concrete commands and configuration locations, while avoiding unsafe system-wide changes.

# Migration plan

Describe compatibility and rollout stages.

# Implementation phases

Break the work into ordered phases. Each phase must include:

- Goal.
- Files/modules likely affected.
- API or data-model changes.
- Security considerations.
- Tests.
- Exit criteria.

# Open decisions

List only decisions that genuinely require user input.

# Final acceptance criteria

Provide a checklist that proves the feature is complete and safe.

Be skeptical, security-first, and explicit about tradeoffs. Do not propose a cloud backend or public tunnel unless you clearly label it as a separate non-local alternative.
