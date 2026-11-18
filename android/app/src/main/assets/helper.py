#!/usr/bin/env python3
# board-power: lets the -board app reboot or shut down this server.
# Remove it with: systemctl disable --now board-power; rm /usr/local/bin/board-power /etc/board-power.key /etc/systemd/system/board-power.service
import hmac, json, os, socket, subprocess, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

KEY = open("/etc/board-power.key").read().strip()
PORT = int(os.environ.get("BOARD_POWER_PORT", "61209"))
CMDS = {"/reboot": ["systemctl", "reboot"], "/shutdown": ["systemctl", "poweroff"]}

class Handler(BaseHTTPRequestHandler):
    def reply(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def authed(self):
        given = self.headers.get("Authorization", "").encode()
        if hmac.compare_digest(given, ("Bearer " + KEY).encode()):
            return True
        time.sleep(1)  # slows down guessing
        self.reply(401, {"ok": False, "error": "wrong key"})
        return False

    def do_GET(self):
        if self.path != "/health":
            return self.reply(404, {"ok": False})
        if self.authed():
            self.reply(200, {"ok": True, "host": socket.gethostname()})

    def do_POST(self):
        cmd = CMDS.get(self.path)
        if not cmd:
            return self.reply(404, {"ok": False})
        if self.authed():
            self.reply(200, {"ok": True})
            threading.Timer(1.0, subprocess.run, [cmd]).start()

    def log_message(self, *args):
        pass

ThreadingHTTPServer(("", PORT), Handler).serve_forever()
