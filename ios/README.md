# -board for iPhone (work in progress)

A native iPhone app version of the Scriptable script, for free sideloading with SideStore / AltStore.

## Status (18 November 2026)

- **Written, never compiled.** No Mac was available: GitHub Actions never assigned a macOS runner to this
  repository (Linux runners worked). Expect a few rounds of compile fixes on the first real build.
- **Not tested on an iPhone.**

## How it's built

- `App/Resources/live.html` is the script's own dashboard page (`LIVE_HTML` from `board_Blank.js`, extracted
  unchanged). `App/Dashboard.swift` hosts it in a WKWebView and answers its `add://`, `del://`, `set://`, `pwr://`,
  `srv://` ... requests exactly as the script's `showLiveDashboard` does.
- `Shared/` is a Swift port of the script's logic (data file, settings cleanup, servers, Glances, logos, up/down
  checks), shared by the app and the widget through the app group. `App/Power.swift` has the Reboot / Shut down drivers.
- `Widget/` is the WidgetKit widget (small / medium / large), with a "Server" option in Edit Widget.
- `project.yml` is an XcodeGen project; `.github/workflows/ios.yml` builds an unsigned `board.ipa`, ad-hoc signs it
  with the app-group entitlements, and publishes it plus a SideStore / AltStore source (`apps.json`, written by
  `make_source.py`) to the `ios-latest` release.

## Next steps

1. Get a macOS build running: GitHub Actions (check the account's Actions / billing settings if macOS jobs stay
   queued), or on a Mac:
   ```sh
   brew install xcodegen
   cd ios && xcodegen generate
   xcodebuild -project Board.xcodeproj -scheme Board -configuration Release -sdk iphoneos \
     -destination 'generic/platform=iOS' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build
   ```
2. Fix the compile errors it reports.
3. Install through SideStore (source: `https://github.com/borissiivanov1-crypto/-board/releases/download/ios-latest/apps.json`)
   and test against a real server.

To regenerate `live.html` after changing the script, extract `LIVE_HTML` again (it is a template literal with no
`${...}` placeholders).
