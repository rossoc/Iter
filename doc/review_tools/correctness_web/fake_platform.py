#!/usr/bin/env python3
"""A minimal time-platform webhook for the 2026-11-02 correctness review.
GET  /time-entries -> {"entries": [], "has_more": false}   (a platform whose listing lags or is empty)
POST /time-entries -> {"results": []}                        (everything accepted)
Every POST body is appended to <logfile> so pushes can be counted.
usage: fake_platform.py <port> <logfile>"""
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT, LOG = int(sys.argv[1]), sys.argv[2]

class H(BaseHTTPRequestHandler):
    def _send(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        if self.path.startswith("/time-entries"):
            return self._send({"entries": [], "has_more": False})
        self.send_response(404); self.end_headers()
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(n).decode()
        with open(LOG, "a") as f:
            f.write(body + "\n")
        self._send({"results": []})
    def log_message(self, *a):
        pass

HTTPServer(("127.0.0.1", PORT), H).serve_forever()
