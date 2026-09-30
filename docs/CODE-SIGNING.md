# Code signing and notarization

Why this matters commercially, and exactly what to do once the Apple Developer
Program is paid for.

## What you get for the $99

Without signing and notarization, macOS shows *"Track2Mix cannot be opened
because the developer cannot be verified"* with **Move to Trash** as the obvious
button. That dialog lands on people who already decided they wanted your app.
It is the largest single drop-off in the funnel.

With notarization, the app opens on a double-click and macOS says nothing. There
is no cheaper conversion win available to a Mac desktop app.

`.github/workflows/release.yml` already reads the secrets below and **builds fine
without them**, producing an unsigned bundle — so nothing blocks shipping before
you enrol.

## Enrol

1. Sign up at [developer.apple.com/programs](https://developer.apple.com/programs/)
   — $99/yr, and approval can take a day or two.
2. In the [Certificates portal](https://developer.apple.com/account/resources/certificates/list),
   create a **Developer ID Application** certificate. That's the one for apps
   distributed outside the App Store; a Mac App Distribution certificate will not
   work here.
3. Download the `.cer`, open it to install it into Keychain Access, then in
   Keychain Access right-click the certificate → **Export** → `.p12`, and set a
   password you'll keep.

## Create an app-specific password for notarization

Notarization authenticates as your Apple ID and **will not accept your real
password**:

1. Go to [account.apple.com](https://account.apple.com) → Sign-In and Security →
   **App-Specific Passwords**.
2. Generate one, label it `notarytool`, and copy it.

## Add the GitHub secrets

Repository → **Settings → Secrets and variables → Actions → New repository
secret**. All six:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | The `.p12`, base64-encoded: `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full identity string, e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_PASSWORD` | The **app-specific** password, not your Apple ID password |
| `APPLE_TEAM_ID` | Ten-character Team ID from the [membership page](https://developer.apple.com/account#MembershipDetailsCard) |

Find the exact signing identity string with:

```bash
security find-identity -v -p codesigning
```

## Ship a release

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow builds macOS (universal) and Windows (x64), signs and notarizes the
macOS bundle when the secrets are present, and opens a **draft** GitHub Release.
Review it, then publish.

## Verify it actually worked

Download the DMG from the release — not your local build, which is already
trusted on your own machine — and on a Mac:

```bash
spctl -a -vvv -t install /Applications/Track2Mix.app
# want: source=Notarized Developer ID

xcrun stapler validate /Applications/Track2Mix.app
# want: The validate action worked!
```

The only test that counts is a genuine one: send the DMG to someone who has never
run a build of yours and watch whether it opens without a warning.

## Windows

Windows signing is a separate purchase and **not worth it yet**. An OV
code-signing certificate runs a few hundred dollars a year, and since 2023 the
private key must live on approved hardware or in a cloud HSM, which adds cost and
setup. An EV certificate clears SmartScreen faster but costs more again.

SmartScreen's warning is milder than Gatekeeper's — **More info → Run anyway** is
right there — and reputation accrues as people download each release. Document it
in `docs/INSTALL.md`, which it already is, and revisit only if Windows becomes a
meaningful share of your users.
