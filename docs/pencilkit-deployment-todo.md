# PencilKit Companion Deployment TODO

This checklist records the native iPad/iPhone companion setup. Completed steps are already checked.

> **The TestFlight route replaces the cable-install route below.** GitHub Actions builds and
> uploads the app, so no Mac needs to be signed into an Apple ID. See
> [TestFlight via GitHub Actions](#testflight-via-github-actions) at the end of this file.
> The "Register iPad device", "Create provisioning profile", "Configure Xcode project for manual
> signing", and "Run on iPad" sections are only needed for direct cable installs.

## Xcode setup

- [x] Install Xcode.
- [x] Point command-line tools at full Xcode:

  ```bash
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
  ```

- [x] Accept the Xcode license:

  ```bash
  sudo xcodebuild -license accept
  ```

- [x] Run first-launch setup:

  ```bash
  sudo xcodebuild -runFirstLaunch
  ```

- [x] Verify Xcode is active:

  ```bash
  xcodebuild -version
  ```

  Observed:

  ```text
  Xcode 26.5
  Build version 17F42
  ```

## Native companion project

- [x] Create `ipad-companion/GoghModeCompanion.xcodeproj`.
- [x] Create SwiftUI app entry point.
- [x] Create PencilKit canvas wrapper.
- [x] Create `DrawingSnapshot` JSON model matching the Rust server schema.
- [x] Create upload client for the existing `/<token>/save` endpoint.
- [x] Create debounced upload controller.
- [x] Create setup/drawing user interface.
- [x] Add local network permission in `Info.plist`.
- [x] Add unit tests for JSON encoding and endpoint normalization.
- [x] Build against iOS simulator SDK.
- [x] Install iOS 26.5 simulator runtime.
- [x] Run companion tests successfully.

## Signing certificate

- [x] Create a certificate signing request from this Mac using OpenSSL:

  ```bash
  DEV_EMAIL="your-developer-email@example.com"
  COMMON_NAME="Desley Langeveld"

  openssl req \
    -new \
    -newkey rsa:2048 \
    -nodes \
    -keyout "$HOME/Downloads/goghmode-apple-development.key" \
    -out "$HOME/Downloads/goghmode-apple-development.certSigningRequest" \
    -subj "/emailAddress=$DEV_EMAIL/CN=$COMMON_NAME/C=NL"
  ```

- [x] In Apple Developer, create a new **Apple Development** certificate from that CSR.
- [x] Download the certificate as `~/Downloads/development-new.cer`.
- [x] Import the private key and certificate into the login keychain:

  ```bash
  security import "$HOME/Downloads/goghmode-apple-development.key" -k "$HOME/Library/Keychains/login.keychain-db"
  security import "$HOME/Downloads/development-new.cer" -k "$HOME/Library/Keychains/login.keychain-db"
  ```

- [x] Verify the certificate and key match:

  ```bash
  openssl x509 -inform DER -in "$HOME/Downloads/development-new.cer" -pubkey -noout > /tmp/goghmode-cert-pubkey.pem
  openssl pkey -in "$HOME/Downloads/goghmode-apple-development.key" -pubout > /tmp/goghmode-key-pubkey.pem
  shasum -a 256 /tmp/goghmode-cert-pubkey.pem /tmp/goghmode-key-pubkey.pem
  ```

- [x] Install Apple Worldwide Developer Relations G3 intermediate certificate:

  ```bash
  /usr/bin/python3 - <<'PY'
  from urllib.request import urlretrieve
  urlretrieve('https://www.apple.com/certificateauthority/AppleWWDRCAG3.cer', '/tmp/AppleWWDRCAG3.cer')
  PY
  security import /tmp/AppleWWDRCAG3.cer -k "$HOME/Library/Keychains/login.keychain-db"
  ```

- [x] Verify valid signing identity:

  ```bash
  security find-identity -v -p codesigning
  ```

  Observed:

  ```text
  Apple Development: Desley Langeveld (M499783J8T)
  1 valid identities found
  ```

## Register iPad device

- [ ] Plug iPad into the Mac with a cable.
- [ ] Unlock iPad and tap **Trust This Computer** if prompted.
- [ ] Open Finder.
- [ ] Select the iPad in Finder sidebar.
- [ ] Click the **Serial Number** text under the device name until it changes to **UDID**.
- [ ] Copy the UDID.
- [ ] Open Apple Developer Devices:

  https://developer.apple.com/account/resources/devices/list

- [ ] Click **+**.
- [ ] Add device:
  - Platform: **iOS, tvOS, watchOS**
  - Name: `Desley iPad`
  - UDID: paste copied UDID
- [ ] Save the device.

## Create App ID

- [ ] Open Apple Developer Identifiers:

  https://developer.apple.com/account/resources/identifiers/list

- [ ] Click **+**.
- [ ] Choose **App IDs**.
- [ ] Choose **App**.
- [ ] Fill in:
  - Description: `GoghMode Companion`
  - Bundle ID: **Explicit**
  - Bundle ID value: `dev.goghmode.companion`
- [ ] Register the App ID.

## Create provisioning profile

- [ ] Open Apple Developer Profiles:

  https://developer.apple.com/account/resources/profiles/list

- [ ] Click **+**.
- [ ] Choose **iOS App Development**.
- [ ] Select App ID: `dev.goghmode.companion`.
- [ ] Select certificate: `Apple Development: Desley Langeveld (M499783J8T)`.
- [ ] Select the registered iPad.
- [ ] Name the profile: `GoghMode Companion Development`.
- [ ] Generate the profile.
- [ ] Download the `.mobileprovision` file.
- [ ] Double-click the `.mobileprovision` file to install it.

## Configure Xcode project for manual signing

- [ ] Open the project:

  ```bash
  open ipad-companion/GoghModeCompanion.xcodeproj
  ```

- [ ] In Xcode, click the blue project icon `GoghModeCompanion`.
- [ ] Select target `GoghModeCompanion`.
- [ ] Open **Signing & Capabilities**.
- [ ] Turn **Automatically manage signing** off.
- [ ] Confirm Bundle Identifier is:

  ```text
  dev.goghmode.companion
  ```

- [ ] Select provisioning profile: `GoghMode Companion Development`.
- [ ] Confirm signing certificate is: `Apple Development: Desley Langeveld (M499783J8T)`.

## Run on iPad

- [ ] Connect iPad to Mac.
- [ ] In Xcode, select the connected iPad as the run destination.
- [ ] Press **Run**.
- [ ] If iPad blocks the app, trust the developer certificate:
  - iPad Settings → General → VPN & Device Management
  - Trust `Desley Langeveld` developer certificate.
- [ ] Open the Mac GoghMode app.
- [ ] Copy the Mac mobile URL.
- [ ] Paste the URL into the iPad companion app.
- [ ] Draw with Apple Pencil.
- [ ] Tap **Save Now**.
- [ ] Verify on Mac that these files update:

  ```text
  drawings/latest.json
  drawings/latest.svg
  drawings/latest.png
  ```

## TestFlight via GitHub Actions

`.github/workflows/ios-testflight.yml` archives and uploads the app from a `macos-26` runner.
Nothing here requires signing a Mac into an Apple ID — every credential comes from the two Apple
web portals plus `openssl`.

Fixed values used throughout:

```text
Team ID:    3J7HD944C3
Bundle ID:  dev.goghmode.companion
Repository: yelsed/goghmode
Profile:    GoghMode Companion App Store
```

The profile name must match `ipad-companion/ExportOptions.plist` and the workflow's
`PROVISIONING_PROFILE_SPECIFIER` character for character.

### Distribution certificate

- [ ] Generate a key and certificate signing request:

  ```bash
  openssl req \
    -new \
    -newkey rsa:2048 \
    -nodes \
    -keyout "$HOME/goghmode-distribution.key" \
    -out "$HOME/goghmode-distribution.certSigningRequest" \
    -subj "/emailAddress=langeveld@fivespark.com/CN=Desley Langeveld/C=NL"
  ```

- [ ] At https://developer.apple.com/account/resources/certificates/list create an
      **Apple Distribution** certificate from that request.
- [ ] Download it as `~/Downloads/distribution.cer`.
- [ ] Assemble the PKCS#12 bundle that the workflow imports:

  ```bash
  openssl x509 -inform DER -in "$HOME/Downloads/distribution.cer" -out /tmp/dist.pem
  curl -sL -o /tmp/AppleWWDRCAG3.cer https://www.apple.com/certificateauthority/AppleWWDRCAG3.cer
  openssl x509 -inform DER -in /tmp/AppleWWDRCAG3.cer -out /tmp/wwdr.pem

  openssl pkcs12 -export -legacy \
    -inkey "$HOME/goghmode-distribution.key" \
    -in /tmp/dist.pem \
    -certfile /tmp/wwdr.pem \
    -name "Apple Distribution: Desley Langeveld" \
    -out "$HOME/goghmode-distribution.p12"
  ```

  `-legacy` is required with OpenSSL 3. Without it the archive uses encryption that macOS
  `security import` cannot read, and the workflow fails at the keychain step.

### App ID and provisioning profile

- [ ] At https://developer.apple.com/account/resources/identifiers/list create an **App ID**:
      description `GoghMode Companion`, explicit Bundle ID `dev.goghmode.companion`.
- [ ] At https://developer.apple.com/account/resources/profiles/list create a profile:
      **App Store Connect** distribution → App ID `dev.goghmode.companion` → the Apple
      Distribution certificate above → name it exactly `GoghMode Companion App Store`.
- [ ] Download the `.mobileprovision` file.

### App Store Connect

- [ ] At https://appstoreconnect.apple.com create the app record: platform iOS, Bundle ID
      `dev.goghmode.companion`, SKU `goghmode-companion`.

  The app name must be globally unique across the App Store even for a TestFlight-only app. If
  `GoghMode Companion` is taken, use `GoghMode Sketch Companion` — this affects only the store
  listing, not the bundle ID or the home-screen name.

- [ ] At https://appstoreconnect.apple.com/access/integrations/api create an API key with the
      **App Manager** role. The `.p8` file is downloadable exactly once. Record the Key ID and
      the Issuer ID.

### GitHub secrets

- [ ] Push all seven secrets:

  ```bash
  REPO=yelsed/goghmode
  gh secret set BUILD_CERTIFICATE_BASE64    --repo "$REPO" < <(base64 -i "$HOME/goghmode-distribution.p12")
  gh secret set PROVISIONING_PROFILE_BASE64 --repo "$REPO" < <(base64 -i "$HOME/Downloads/GoghMode_Companion_App_Store.mobileprovision")
  gh secret set ASC_API_KEY_BASE64          --repo "$REPO" < <(base64 -i "$HOME/Downloads/AuthKey_XXXXXXXXXX.p8")
  gh secret set ASC_API_KEY_ID              --repo "$REPO"   # paste the Key ID
  gh secret set ASC_API_ISSUER_ID           --repo "$REPO"   # paste the Issuer ID
  gh secret set P12_PASSWORD                --repo "$REPO"   # the password used in pkcs12 -export
  gh secret set KEYCHAIN_PASSWORD           --repo "$REPO"   # any string; scratch keychain only
  ```

- [ ] Delete the local `.key`, `.p12`, `.p8`, `.cer`, and `.mobileprovision` files, or move them
      into a password manager.

### Run the deploy

- [ ] Trigger and watch:

  ```bash
  gh workflow run ios-testflight.yml --repo yelsed/goghmode
  gh run watch --repo yelsed/goghmode
  ```

- [ ] In App Store Connect → TestFlight, wait for the build to finish processing.
- [ ] Create an **Internal Testing** group and add yourself. Internal testing needs no beta review.
- [ ] Install Apple's TestFlight app on the iPad, accept the invite, install the companion.

The build number comes from `github.run_number`, so every run is unique automatically. TestFlight
builds expire after 90 days — re-run the workflow to refresh.
