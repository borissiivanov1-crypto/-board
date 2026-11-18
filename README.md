# -board

A live home-server dashboard and home screen widget for [Scriptable](https://scriptable.app) on iOS.
The whole app is one file: `board_Blank.js`.

**Android:** a native version (Kotlin + Jetpack Compose) is in [`android/`](android/). Install
[`android/dist/board.apk`](android/dist/board.apk); see [Android app](#android-app) below.

The full step-by-step guide is the [iOS user manual](board_User_Manual_iOS.docx) (Word document).

## Works with

-board needs your **server's IP or name** (Settings > Server). The **LIVE / OFFLINE** status shows whether anything
on the server answers: its web ports, the server system's own address, Glances and your service buttons.

**Monitoring** (CPU, RAM, drives, network, temperatures) comes from Glances and works on every server. It's optional:
turn it off in *Settings > Monitoring* and the Monitoring panel and widget gauges go away, and -board sends no requests
to Glances (the widget then shows your service buttons). The panel is folded away by default.

Reboot / Shut down depends on the system the server runs:

| Server | Reboot / Shut down |
| --- | --- |
| **ZimaOS** | ZimaOS login. Nothing to install |
| **Proxmox VE** | Proxmox API (port 8006) with your Proxmox login, e.g. `root@pam`. Nothing to install |
| **TrueNAS** (SCALE / Community Edition, CORE) | TrueNAS login, over the WebSocket JSON-RPC API on 25.04 and newer, the REST API on older versions. Nothing to install |
| **Unraid** | Unraid login (`root`); sends what Unraid's own Reboot / Shut down buttons send. Nothing to install |
| **Synology DSM** | DSM Web API (port 5000 / 5001) with an administrator account. Nothing to install |
| **OpenMediaVault** | OpenMediaVault's API with an administrator account (`admin`). Nothing to install |
| **Linux** (Debian, Ubuntu Server, Fedora ...) | A tiny helper service. Settings gives you a one-time setup command |

**Pick your server type** in *Settings > Server > Server type* (ZimaOS, Proxmox, TrueNAS, Unraid, Synology, OMV or Linux).
A new install has none picked, so there's no Reboot / Shut down menu until you choose one. Settings then shows
only that system's options.

Accounts with two-factor login can't be used for Reboot / Shut down; make a separate administrator account for -board.
Logins are stored in the iOS Keychain. Enter a login ahead of time with **Enter login** under the server type in Settings
(it is checked against the server before it is saved), or wait to be asked the first time you use Reboot / Shut down.

## Several servers

Swipe the server name left or right to switch servers. The dots under the name show how many there are and which
one is on screen. Swipe left past the last one (or tap **+**) to add a server: it asks the same as Settings, in the same
order (name, server type, server address, and the login if the type uses one, checked before the server is added).

- Each server has its **own settings window** (the gear, titled with the server's name): server address and type,
  logins, Monitoring / Glances, and its own buttons, stacks and layout.
- Theme & look, the widget's refresh and rows, button options and export / import are **shared by all servers**
  (shown under "Shared by all servers" in every settings window).
- A **widget** shows one server: long-press it, **Edit Widget**, and type the server's name or number (`1`, `2` ...)
  under **Parameter**. Empty = the first server. Tapping the widget opens the dashboard on that server.
- **Delete server** is at the very bottom of each server's settings and asks to confirm first (your only server can't be deleted).
  Export / import covers every server.

Existing setups become server 1 with everything kept, including saved logins.

## 1. Run Glances on the server (optional, for Monitoring)

Docker (any OS):

```sh
docker run -d --name glances --restart unless-stopped -p 61208:61208 --pid host \
  -e GLANCES_OPT="-w" -v /var/run/docker.sock:/var/run/docker.sock:ro nicolargo/glances
```

Without Docker (Debian, Ubuntu, Proxmox host): `sudo apt install glances`, then run `glances -w`
(for example as a systemd service). Distro packages are often Glances 3.x, so pick **API version 3** in Settings.
*Test connection* tells you if the other API version answers.

## 2. Install the script

1. In Scriptable tap **+**, paste `board_Blank.js` and give it a name.
2. Run it. Settings opens: under **Server**, give it a name, pick your **server type**, and type the **server address**:
   an http / https list plus the IP or name, with a port only if it isn't the usual one for that type
   (e.g. `192.168.1.10` or `192.168.1.10:90`). This one address is used for LIVE / OFFLINE, Reboot / Shut down and logins.
   Tap **Enter login** if the type uses one. Under **Monitoring**, leave the Glances address empty if Glances runs on
   the same server on port 61208, or turn Monitoring off if you don't use Glances. Tap **Test connection**, then **Save**.
3. Add a Scriptable widget to your home screen and pick this script.

## Looks

*Settings > Theme & look* has two optional extras, both off by default:

- **Style**: *Classic* (the plain -board look) or *Liquid Glass* (iOS 26 style glass cards with a bright rim,
  a third background glow, springy animation on presses, menus, sheets and drags, and glass rings and pills on the widget).
  The change previews live behind the Settings sheet.
- **Startup animation**: a short animated -board logo when the dashboard opens (tap to skip). Works with either style.

Themes, accent colours, glow, corners, font and button size work with both styles.

## Reboot / Shut down per server type

- **ZimaOS**: you're asked for your ZimaOS login the first time; it's saved in the iOS Keychain.
- **TrueNAS**: your admin login (`truenas_admin`, or `root` on older versions). Use plain http on your home network or VPN,
  or https with a certificate the iPhone trusts (TrueNAS's own self-signed certificate doesn't work for its WebSocket API).
- **Unraid**: `root` and your password. If Unraid redirects to an `https://...myunraid.net` address, -board tells you to use that as the server address.
- **Synology**: an administrator account without two-factor login. Pick https in the server address and port 5001 is used.
- **OpenMediaVault**: an administrator account, `admin` by default.
- **Proxmox**: you're asked for your Proxmox login (`root` or `root@pam`; any user with the `Sys.PowerMgmt` permission works).
  Accounts with two-factor login can't be used. In a cluster, enter the node name in Settings (otherwise it's found automatically).
- **Linux**: tap **Create key & copy setup command**, SSH into the server, paste and press Enter. It asks for sudo once and:
  - installs `/usr/local/bin/board-power` (one Python 3 file, standard library only),
  - stores the random secret key in `/etc/board-power.key` (readable by root only),
  - starts the `board-power` systemd service on port 61209 and opens that port in ufw / firewalld if they're active.

  The helper only answers `/health`, `/reboot` and `/shutdown`, and only with the right key. Keep it on your LAN or VPN.
  To remove it:

  ```sh
  sudo systemctl disable --now board-power
  sudo rm /usr/local/bin/board-power /etc/board-power.key /etc/systemd/system/board-power.service
  ```

## Android app

[`android/`](android/) is a native Android port of the script (Kotlin, Jetpack Compose, a Jetpack Glance home screen widget)
with the same features, logic and look: several servers, LIVE / OFFLINE, Glances monitoring with the enlarged cards
(history graph, per-core load, top processes, drives, temperatures), service buttons with up/down dots, stacks,
drag to rearrange, Widget layout, Reboot / Shut down for ZimaOS, Proxmox, TrueNAS, Unraid, Synology, OpenMediaVault
and Linux (the same helper setup command), every theme and accent, Classic and Liquid Glass, the startup animation,
and Settings with Test connection.

**Install:** copy [`android/dist/board.apk`](android/dist/board.apk) to the phone and open it (allow installing from
that source when asked). Android 8.0 or newer. The APK is signed with a development key, so it installs
directly but isn't from the Play Store.

Differences from the iPhone version:
- **Settings move between the two.** *Settings > Data > Export* on either one copies the same format, and *Import*
  on the other reads it (all servers, buttons, stacks and layout; passwords stay on each phone).
- Logins are stored encrypted on the phone (Android Keystore) instead of the iOS Keychain.
- **Widget:** add it from the home screen's widget list. It asks which server to show (the Scriptable Parameter);
  long-press it and pick *Reconfigure* to change it on Android 12 and newer. Resize it for the small, medium or
  large layout. Android refreshes widgets at most every 15 minutes in the background; opening -board refreshes them too.
- Self-signed https certificates are accepted for every request (as the script does for the server systems).
- The *Rounded* font option uses Android's sans-serif (SF Pro Rounded is Apple-only).

**Build it yourself:** with JDK 17+ and the Android SDK (platform 35), run `./gradlew assembleRelease` in `android/`;
the APK is `app/build/outputs/apk/release/app-release.apk`. `./gradlew testReleaseUnitTest` runs the unit tests.
