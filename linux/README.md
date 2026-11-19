# -board for GNOME

The Linux desktop version of [-board](../README.md): the same live home-server dashboard, written in
Rust with GTK4 and libadwaita. It follows the latest version of the app: the features, settings, server
types and looks of the Scriptable script, plus the Android app's ZimaOS integration. It reads the same
export files as the iOS and Android versions, so you can move your setup between them.

| | |
| --- | --- |
| Dashboard | LIVE / OFFLINE, fold-away Monitoring panel (CPU, RAM, storage rings, network), service buttons with up / down dots, stacks, several servers |
| Enlarged cards | Click a card: user / system / IO wait, load, per-core bars, temperatures, history graph, top processes, drives with read / write speed |
| Reboot / Shut down | ZimaOS, Proxmox VE, TrueNAS, Unraid, Synology DSM, OpenMediaVault, Linux (with the small helper service) |
| ZimaOS | Start / stop / restart apps, app logs, app updates, add apps from ZimaOS, CPU power draw, drive health |
| Desktop widget | The iOS widget's small, medium and large layouts in a small borderless window |
| Looks | Classic or Liquid Glass, 7 themes, accent colours, glow, card style, corners, font, button size, startup animation |

## Install

Needs GTK 4.14+ and libadwaita 1.5+ (Ubuntu 24.04, Debian 13, Fedora 40 and newer) and Rust 1.83+ ([rustup](https://rustup.rs)).

```sh
# Ubuntu / Debian
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libdbus-1-dev
# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel dbus-devel
# Arch
sudo pacman -S base-devel gtk4 libadwaita dbus

cd linux
make                          # builds target/release/board
make install PREFIX=~/.local  # or: sudo make install (to /usr/local)
```

`make install` adds **-board** to the app grid, with its icon, and the small / medium / large widgets
in the icon's right-click menu. `make test` runs the tests.

## Use

Everything works as in the [iOS user manual](../board_User_Manual_iOS.docx). On the desktop:

| iOS | GNOME |
| --- | --- |
| Tap | Click |
| Press and hold a button (edit, delete, add to stack) | Hold it, or right-click it |
| Press, hold and drag to rearrange | Drag it (onto another button, into an open stack, or between the two Widget layout panels) |
| Hold the server name to rename it | Hold it, or right-click it |
| Swipe the server name | Swipe it, two-finger swipe on it, **Ctrl+Page Down / Page Up**, or **Alt+→ / ←** |
| Gear icon | Gear icon, or **Ctrl+,** |
| **+** to add a service | **+**, or **Ctrl+N** |
| | **Escape** closes a popup, **F5** checks everything again now |
| iOS Keychain | The GNOME keyring (Secret Service). Without a keyring, a private file only you can read (`~/.local/share/board/secrets.json`, mode 600) |
| Home screen widget | `board --widget small|medium|large [--server NAME|NUMBER]`, the buttons in *Settings > Widget*, or the app icon's right-click menu |

**Desktop widget**: click a service to open it, a stack for the dashboard with that stack open, anywhere
else for the dashboard on that server. Drag it to move it; right-click it to change its size, refresh it
or close it. It refreshes every *Refresh every (minutes)*. To have it at login, add
`board --widget large` to *Startup Applications* (GNOME Tweaks).

`board --render-widget widget.png --size large` draws a widget to a PNG file and quits (no window),
for desktop tools like Conky.

### ZimaOS integration (ZimaOS servers only)

Only on servers whose type is **ZimaOS**, and only with the ZimaOS login saved (*Settings > Server > Enter login*).
It works as in the Android app:

- **Start, stop and restart apps:** hold (or right-click) a button that points at a ZimaOS app. The menu shows whether
  it's running and gives **Start** or **Stop** (Stop asks for a second click), **Restart**, **Logs**, and **Update** when
  the App Store has a newer version. Buttons are linked to apps by port (a button on the server's address and the port
  of exactly one app), or by the app they were added from. Apps are checked once a minute.
- **App logs:** the last 300 lines, with **Refresh**.
- **App updates:** a **↑ 2 updates** chip next to *Widget layout* lists the apps with newer versions, each with **Update**.
- **Add apps from ZimaOS:** the **+** popup has **Pick from ZimaOS apps**; apps that already have a button are greyed out.
- **CPU power draw** in the enlarged CPU card (Intel RAPL, from the second reading on), and ZimaOS's CPU temperature
  when Glances has none.
- **Drive health** in the enlarged storage card: each drive's SMART check (**OK** / **FAILING**) and temperature, also
  used for drives Glances has no temperature for.

### Command line

```text
board [--server NAME|NUMBER] [--stack ID] [--settings]
board --widget [small|medium|large] [--server NAME|NUMBER]
board --render-widget FILE [--size small|medium|large] [--server NAME|NUMBER]
```

### Where things are kept

| What | Where |
| --- | --- |
| Servers, buttons, stacks, layout, settings | `~/.local/share/board/board-data.json` (the same format as the iOS version's) |
| Logins, the helper key, Glances passwords | The GNOME keyring (service `board`), same names as the iOS Keychain |
| Logos | `~/.cache/board/logos` |

`BOARD_DATA_DIR=/some/folder board` keeps everything in one folder instead (handy for a second setup or testing).

**Moving from iPhone**: on the iPhone, *Settings > Data > Export settings*, get the text to this computer
(e.g. AirDrop or a note), copy it, then *Settings > Data > Import settings* here. Passwords aren't exported:
enter the logins again (*Settings > Server > Enter login*) and run the Linux helper's setup command again
(or copy the key).

## Differences from the iOS version

- GTK can't blur what's behind a panel, so popups are a little more opaque, and the Liquid Glass panels use
  a tinted glass colour instead of a blurred one. Everything else in the look (gradients, rims, springy
  pop-ins and presses, the glass ring lenses, the moving glows) is drawn the same way.
- Requests to your server go straight to it, never through a system proxy (a proxy could answer for a
  server that's down). Logo downloads from the internet use the system proxy.
- TrueNAS over https needs a certificate this computer trusts (as on iOS).

## Code

| File | What |
| --- | --- |
| `src/settings.rs` | Defaults, cleaning of saved / imported / typed settings, addresses |
| `src/store.rs` | The data file: servers, migration from the single-server format, import / export |
| `src/layout.rs` | Buttons, order, stacks, the widget's cells |
| `src/glances.rs` | Monitoring: stats, drives, network, temperatures, card details |
| `src/net.rs` | HTTP, up / down checks, LIVE / OFFLINE |
| `src/power.rs` | Reboot / Shut down for every server type, the Linux helper and its setup command |
| `src/zima.rs` | ZimaOS integration: apps, logs, updates, CPU power, drive health |
| `src/secrets.rs`, `src/logos.rs` | Saved logins; the logo cache |
| `src/ui/` | The GTK4 / libadwaita interface: dashboard, popups, Settings, drawing, desktop widget, startup animation |

The tests (`make test`) cover settings cleaning, data migration and import / export, stacks and ordering,
Glances parsing, the up / down checks against real sockets, every Reboot / Shut down driver against fake
servers (including a TrueNAS JSON-RPC WebSocket), the ZimaOS integration against a fake ZimaOS, the helper script, the logo cache, the widget drawing and
the drag-and-drop rules.
