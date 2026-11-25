# -board: notes for Claude sessions

-board is a home-server dashboard and home screen widget. Two versions:
- **iOS / Scriptable**: released as numbered folders `ios-X.Y/`, each with `-board-ios-X.Y.js` (the whole script) and
  `board_User_Manual_iOS.docx`. **The latest is `ios-2.4/`.** For a new iOS version: copy the latest folder to
  `ios-X.Y/`, rename the script to `-board-ios-X.Y.js`, set `BOARD_VERSION`, update the manual's date/version line,
  "What's new" and the file name in "Copy the script" and the requirements table, and the README's iOS paths; commit,
  push, send both files. (Before 2.4 the script was the unnumbered `board_Blank.js`.)
- **Android** (Kotlin, Jetpack Compose, Glance widget): released as numbered folders `android-X.Y/`, each with
  `board-X.Y.apk`, `board-android-source-X.Y.zip` and `board_User_Manual_Android.docx`.
  **The latest is `android-2.4/`. Always start from the newest zip** (unzip it into a scratch folder; it contains
  `board-android-source/android/...` and the manual and README).

## How the user works
- Requests come as bullet lists of fixes, ending with "build me the new source code and apk file as vX.Y and update
  the android user manual and send them here". For that:
  1. bump `versionCode` (+1) and `versionName` in `android/app/build.gradle.kts` (2.4 = versionCode 15);
  2. make the changes, build, run the unit tests;
  3. update the manual (add the new version's items to the "What's new in 1.3 to X.Y" section, set the date/version
     line under the title, fix every section the change touches) and the README's "New in Android X.Y" section;
  4. package: put the new APK at `android/dist/board.apk` inside the source, zip as `board-android-source-X.Y.zip`
     (top folder `board-android-source/`, no build/, .gradle/, .kotlin/, local.properties), then rebuild from the
     packaged copy to check it builds on its own;
  5. write `android-X.Y/` in this repo, commit, push to the session's branch, and **send the three files** with
     SendUserFile;
  6. reply with a short summary of what changed. Say plainly that it wasn't tried on a phone (no emulator here).
- Keep user-facing text plain and short (hints, toasts, manual). The manual writes the name as "‑board" with a
  non-breaking hyphen (U+2011).

## Building (cloud container)
- JDK 21, Android SDK at `/opt/android-sdk` (platform 35), Gradle at `/opt/gradle-8.14.3/bin/gradle`.
- In `android/`: `echo "sdk.dir=/opt/android-sdk" > local.properties`, then
  `/opt/gradle-8.14.3/bin/gradle --no-daemon assembleRelease testReleaseUnitTest -q`
  (add `--offline` once the Gradle cache is filled). APK: `app/build/outputs/apk/release/app-release.apk`.
- Maven Central sometimes answers 429 through the proxy on the first fetch: wait and retry with `--max-workers=2`.
- Check the APK with `/opt/android-sdk/build-tools/35.0.0/aapt2 dump badging` and `apksigner verify --print-certs`
  (signer must be `CN=-board development, O=-board`).

## Signing
- Release builds are signed with the project key `android/keystore/board-dev.jks` (password `board-dev`, alias
  `board`), so every build installs over the last one. Never switch back to the machine's debug key.
- 1.2 was signed with someone else's debug key: going 1.2 → 1.3+ needs Export → uninstall → install → Import
  (documented in the manual and README).

## Editing the Android manual
- Edit the latest `board_User_Manual_Android.docx` in place with python-docx so styles stay identical; helpers are in
  `tools/docx_edit.py` (`find`, `setp` with `**bold**`, `after`/`before` copying a paragraph's style, `cell`,
  `addrow`, `delrow`). Bullets: copy the style of the paragraph starting "Home screen widget that you can resize";
  body text: "It is battery-friendly"; Heading 2: "Add the widget".
- Check the result by converting to PDF (`soffice --headless --convert-to pdf`) and looking at a page or two.

## Android code map (app/src/main/java/app/board)
- `BoardVM.kt`: ViewModel. Live loop (Glances first, LIVE probes skipped while Glances answers on the main address),
  pull-to-refresh (`refreshNow`), reboot watch (`powerWatch`, `watchTick`), Settings form + autosave
  (`openSettings`, `formChanged`, `applyForm`, `closeSettings`), widget snapshot on leaving the dashboard.
  The open popup (`dlg`), `delName` and the Settings sheet (`sheet: SettingsForm`) live here.
- `SettingsForm.kt`: form state, `validateForm` (invalid values are reverted to saved ones and listed as problems),
  `TestLine`/`Fix` (one-tap fixes), page names.
- `ui/SettingsSheet.kt`: pages This server (Server, Monitoring, Test connection, Delete server), Widget, Buttons,
  Appearance, Backup, About. Everything saves as it changes; header has Cancel (`cancelSettings`: puts back the settings
  as they were at open) and Save; "Discard changes?" only for values that can't be saved. Popups (`Modal`) ignore taps
  outside since 1.7: they close with their own buttons or Back.
- `Store.kt` + `DataFile.kt`: `board-data.json`, saved via .tmp + .bak + atomic rename; a damaged file is restored
  from the backup and kept aside, never saved over (`Store.loadNote`, `Store.readOnly`).
- `AutoLogo.kt` (2.2): one logo lookup for Auto and every Pick from (dashboard-icons first via the weekly
  `tree.json` / `metadata.json` index and loose name matching; the server's own icon only as a fallback).
- `Certs.kt` (2.1): trust on first use for https. `PinTrust` (system trust + name, else `CertStore.check` per
  host:port), `CertChanged` -> `CertStore.asked` -> `CertAskDialog` in `ui/App.kt`. The checks client keeps trust-all.
- `Net.kt`: two OkHttp dispatchers: main (24 per host) and checks (32 per host) used by `pingUrl`/`reachProbe`.
- `Ctr.kt`: containers on non-ZimaOS types (hold menu Start / Stop / Restart / Logs): one driver per type (Proxmox,
  TrueNAS, Synology, Unraid, helper for OMV/Linux), `Ctrs.match` links a button by hand-picked `ctr`, address, port or
  name; a `CtrErr.fix` makes the menu show a setup button (`BoardVM.openSetup` scrolls Settings there). "Pick from ..." in the + popup (`CtrAddDialog`, `BoardVM.addCtrButtons`) adds containers as linked buttons. ZimaOS keeps its own code in `Zima.kt`. 2.3 adds the ZimaOS App Store there (`store` / `install` / `uninstall`; `ZimaStoreDialog`, `BoardVM.zInstall`, Uninstall in the hold menu); ported to iOS in 2.4. Helper v2 (`assets/helper.py`) has the `/containers` endpoints.
- `Widget.kt` + `WidgetSnap.kt`: Glance widget; uses the dashboard's snapshot when it's under 3 minutes old and was
  taken with the same settings; otherwise asks Glances first and probes LIVE only if Glances doesn't answer.
- `ui/Look.kt`: press feedback (`pressScale`, `squish`, `glint`): Liquid Glass squish, Classic shrink + dim.
- `ui/Components.kt`: `Modal` (Back / scrim, `onBack`, `scrollKey`), `DiscardAsk`, `Haptic`.
- Manifest: `MainActivity` handles configuration changes itself, so rotation / dark mode / font size don't recreate it.
- Unit tests in `app/src/test/java/app/board` (DataFileTest, NetTest, FailoverTest, LogicTest, ZimaTest, CtrTest, CertTest, AutoLogoTest).

## iOS script
- The script (`ios-X.Y/-board-ios-X.Y.js`) saves the same safe way (`safeWrite`, `readDataFile` with `.tmp` / `.bak`, `LOAD_NOTE` alert).
  Exports move between the iOS and Android versions (same JSON shape).
- **Keep the iOS script pure ASCII** (a copy read as Latin-1 showed `Â·` / `â€º`): write other characters as
  `\u00B7` escapes (`\\u00B7` inside `LIVE_HTML`, a template literal; plain `\u00B7` in `LIVE_EXTRA` and the script).
  Check with `grep -nP "[^\x00-\x7F]" ios-X.Y/-board-ios-X.Y.js`. `unGarble` repairs saved text from such a copy.
- Since the 2.4 port it has the same features as Android 2.4 (`BOARD_VERSION`), except trust-on-first-use
  certificates, haptics and autosave (Scriptable can't). Layout: Scriptable side (Request, Keychain, files) plus a
  WebView page (`LIVE_HTML`, a template literal: double every backslash) and the newer page code in `LIVE_EXTRA`
  (`String.raw`, injected at load). Page -> script: `location.href = "xxx://go?d=..."` caught in `shouldAllowRequest`
  (`set://` Settings, `pwr://`, `rpc://` -> `rpcOp` -> `rpcReply` for apps / containers / logos / refresh); script ->
  page: `wv.evaluateJavaScript`. Container drivers, ZimaOS, AutoLogo, `uniqueName`: the "Apps and containers" section.
- The iOS manual writes "-board" with a plain hyphen. No phone here: check with `node --check`, and run the page in
  Chromium with a mock Scriptable (keep such test scripts in the scratchpad, not the repo).
