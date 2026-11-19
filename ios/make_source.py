#!/usr/bin/env python3
"""Writes apps.json: the SideStore / AltStore source that lists the latest -board build."""
import datetime, json, os, sys

ipa, version, build, out = sys.argv[1:5]
repo = os.environ.get("GITHUB_REPOSITORY", "borissiivanov1-crypto/-board")
base = f"https://github.com/{repo}/releases/download/ios-latest"
source = {
    "name": "-board",
    "identifier": "app.board.source",
    "subtitle": "Home server dashboard",
    "sourceURL": f"{base}/apps.json",
    "iconURL": f"{base}/icon.png",
    "website": f"https://github.com/{repo}",
    "apps": [{
        "name": "-board",
        "bundleIdentifier": "app.board",
        "developerName": "-board",
        "subtitle": "Live server dashboard and widget",
        "localizedDescription": "A live dashboard and home screen widget for your home server: CPU, RAM, storage and network from Glances, "
            "your services with up/down status, stacks, Reboot / Shut down for ZimaOS, Proxmox, TrueNAS, Unraid, Synology, "
            "OpenMediaVault and Linux, several servers, themes and a Liquid Glass look. The same app as the Scriptable script, "
            "with settings you can export and import between them.",
        "iconURL": f"{base}/icon.png",
        "tintColor": "34e0a1",
        "category": "utilities",
        "versions": [{
            "version": version,
            "buildVersion": build,
            "date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "localizedDescription": f"Build {build}.",
            "downloadURL": f"{base}/board.ipa",
            "size": os.path.getsize(ipa),
            "minOSVersion": "17.0",
        }],
        "appPermissions": {
            "entitlements": ["com.apple.security.application-groups"],
            "privacy": {"NSLocalNetworkUsageDescription": "-board connects to your home server and its services on your network."},
        },
    }],
    "news": [],
}
json.dump(source, open(out, "w"), indent=2)
