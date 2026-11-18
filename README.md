# -board

A live home-server dashboard and home screen widget for [Scriptable](https://scriptable.app) on iOS.
The whole app is one file: `board_Blank.js`.

## Works with

| Server | Monitoring (CPU, RAM, drives, network, temperatures) | Reboot / Shut down |
| --- | --- | --- |
| **ZimaOS** | Glances | ZimaOS login, nothing to install |
| **Proxmox VE** | Glances | Proxmox API (port 8006) with your Proxmox login, e.g. `root@pam`. Nothing to install |
| **Linux** (Debian, Ubuntu Server, Fedora, ...) | Glances | A tiny helper service. Settings gives you a one-time setup command |

Pick your server type with the **Server type** radio buttons in *Settings > Server*. Only the Reboot / Shut down settings change; monitoring works the same everywhere.

## 1. Run Glances on the server

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
2. Run it. Settings opens: enter the Glances address (e.g. `http://192.168.1.10:61208`), choose your **server type**, tap **Test connection**, then **Save**.
3. Add a Scriptable widget to your home screen and pick this script.

## Reboot / Shut down per server type

- **ZimaOS**: you're asked for your ZimaOS login the first time; it's saved in the iOS Keychain.
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
