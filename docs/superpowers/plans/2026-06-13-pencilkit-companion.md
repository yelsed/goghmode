# PencilKit Companion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native SwiftUI/PencilKit iPad and iPhone companion app that captures Apple Pencil or touch input and uploads the current drawing to the existing GoghMode Mac bridge.

**Architecture:** Keep the Rust Mac app as the local bridge and output owner. Add a small Xcode iOS app under `ipad-companion/` with a PencilKit canvas, endpoint settings, a snapshot converter, and a debounced upload client that posts the existing `DrawingSnapshot` JSON shape to `/<token>/save`.

**Tech Stack:** SwiftUI, PencilKit, UIKit bridge via `UIViewRepresentable`, Foundation `URLSession`, XCTest, existing Rust HTTP save endpoint.

---

## File structure

- Create `ipad-companion/GoghModeCompanion.xcodeproj/project.pbxproj`: Xcode project with one iOS app target and one unit test target.
- Create `ipad-companion/GoghModeCompanion/GoghModeCompanionApp.swift`: SwiftUI app entry point.
- Create `ipad-companion/GoghModeCompanion/ContentView.swift`: Endpoint setup, status display, and full-screen drawing surface.
- Create `ipad-companion/GoghModeCompanion/PencilCanvasView.swift`: `PKCanvasView` wrapper and drawing change delegate.
- Create `ipad-companion/GoghModeCompanion/DrawingSnapshot.swift`: Codable snapshot schema and PencilKit-to-GoghMode conversion.
- Create `ipad-companion/GoghModeCompanion/GoghModeClient.swift`: Validates endpoint URLs and uploads snapshots.
- Create `ipad-companion/GoghModeCompanion/UploadController.swift`: Debounces drawing changes and updates save status.
- Create `ipad-companion/GoghModeCompanion/Info.plist`: iOS app metadata and local network usage text.
- Create `ipad-companion/GoghModeCompanionTests/DrawingSnapshotTests.swift`: Pure unit coverage for endpoint validation and JSON shape.

## Task 1: Project scaffold

**Files:**
- Create: `ipad-companion/GoghModeCompanion.xcodeproj/project.pbxproj`
- Create: `ipad-companion/GoghModeCompanion/Info.plist`

- [ ] **Step 1: Create the Xcode project**

Create a minimal iOS Xcode project with:

- Bundle ID: `dev.goghmode.companion`
- App target: `GoghModeCompanion`
- Test target: `GoghModeCompanionTests`
- Deployment target: iOS 17.0
- Supported device families: iPhone and iPad
- Swift version: 5.0

- [ ] **Step 2: Create the app Info.plist**

Include `NSLocalNetworkUsageDescription` because the app posts to the Mac over the local network.

- [ ] **Step 3: Open in Xcode**

Run: `open ipad-companion/GoghModeCompanion.xcodeproj`

Expected: Xcode opens the project and shows the app and test targets.

## Task 2: Snapshot model and upload client

**Files:**
- Create: `ipad-companion/GoghModeCompanion/DrawingSnapshot.swift`
- Create: `ipad-companion/GoghModeCompanion/GoghModeClient.swift`
- Create: `ipad-companion/GoghModeCompanionTests/DrawingSnapshotTests.swift`

- [ ] **Step 1: Add Codable schema**

Define `DrawingSnapshot`, `CanvasSize`, `Stroke`, and `Point` to match the Rust server schema exactly:

```swift
struct DrawingSnapshot: Codable, Equatable {
    let schemaVersion: Int
    let canvas: CanvasSize
    let strokes: [Stroke]
}

struct CanvasSize: Codable, Equatable {
    let width: Double
    let height: Double
    let background: String
}

struct Stroke: Codable, Equatable, Identifiable {
    let id: String
    let color: String
    let width: Double
    let points: [Point]
}

struct Point: Codable, Equatable {
    let x: Double
    let y: Double
    let pressure: Double
    let t: UInt64
}
```

- [ ] **Step 2: Add endpoint validation**

Implement `GoghModeEndpoint` so pasted URLs ending in either `/<token>/` or `/<token>/save` normalize to the save URL.

- [ ] **Step 3: Add unit tests**

Cover JSON key names, URL normalization, and rejecting non-HTTP URLs.

- [ ] **Step 4: Run unit tests**

Run: `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test -project ipad-companion/GoghModeCompanion.xcodeproj -scheme GoghModeCompanion -destination 'platform=iOS Simulator,name=iPad Pro 11-inch (M4)'`

Expected: tests pass after the license and simulator runtime are configured.

## Task 3: PencilKit canvas

**Files:**
- Create: `ipad-companion/GoghModeCompanion/PencilCanvasView.swift`
- Modify: `ipad-companion/GoghModeCompanion/DrawingSnapshot.swift`

- [ ] **Step 1: Add `PKCanvasView` wrapper**

Use `UIViewRepresentable` to embed PencilKit in SwiftUI. Configure:

- `drawingPolicy = .anyInput`
- white background
- ink tool with black pen
- `alwaysBounceVertical = false`
- `alwaysBounceHorizontal = false`

- [ ] **Step 2: Convert `PKDrawing` to snapshot**

Implement conversion by iterating over `drawing.strokes`, then interpolating each `PKStrokePath` by distance to produce points. Use `PKStrokePoint.force` as pressure where available. Clamp canvas size to at least 1 point.

- [ ] **Step 3: Keep conversion deterministic**

Use stable stroke IDs based on stroke index: `stroke-1`, `stroke-2`, and so on. Use point offset order for `t`.

## Task 4: SwiftUI user flow

**Files:**
- Create: `ipad-companion/GoghModeCompanion/GoghModeCompanionApp.swift`
- Create: `ipad-companion/GoghModeCompanion/ContentView.swift`
- Create: `ipad-companion/GoghModeCompanion/UploadController.swift`

- [ ] **Step 1: Add app entry point**

Launch `ContentView` from `GoghModeCompanionApp`.

- [ ] **Step 2: Add endpoint setup**

Show a text field for the Mac URL. Store the last valid URL in `UserDefaults`.

- [ ] **Step 3: Add drawing screen**

Show the PencilKit canvas full-screen once an endpoint exists. Keep a top toolbar with endpoint status, Save Now, Clear, and Settings.

- [ ] **Step 4: Add debounced upload**

On every drawing change, schedule upload after 600 ms. Cancel and reschedule while the user keeps writing.

- [ ] **Step 5: Add manual save**

Save Now immediately converts and posts the current drawing.

## Task 5: Verification

**Files:**
- All new files under `ipad-companion/`

- [ ] **Step 1: Accept Xcode license if needed**

Run in a terminal with administrator access:

```bash
sudo xcodebuild -license accept
sudo xcodebuild -runFirstLaunch
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
```

Expected: `xcodebuild -version` prints the installed Xcode version without a license error.

- [ ] **Step 2: Build app**

Run:

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild build -project ipad-companion/GoghModeCompanion.xcodeproj -scheme GoghModeCompanion -destination 'generic/platform=iOS Simulator'
```

Expected: build succeeds.

- [ ] **Step 3: Run tests**

Run:

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test -project ipad-companion/GoghModeCompanion.xcodeproj -scheme GoghModeCompanion -destination 'platform=iOS Simulator,name=iPad Pro 11-inch (M4)'
```

Expected: unit tests pass.

- [ ] **Step 4: Manual device check**

Open the project in Xcode, select the connected iPad or iPhone, choose a development team, run the app, paste the Mac mobile URL, draw with Apple Pencil or finger, tap Save Now, and confirm `drawings/latest.png` updates on the Mac.
