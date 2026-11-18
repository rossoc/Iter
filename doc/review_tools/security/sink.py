# Logs every request (line + headers) it receives; stands in for a foreign platform host.
import http.server, sys, json
class H(http.server.BaseHTTPRequestHandler):
    def _log(self):
        sys.stderr.write(json.dumps({"method": self.command, "path": self.path, "headers": dict(self.headers)}) + "\n"); sys.stderr.flush()
        body = json.dumps({"data": [{"id": 1, "name": "sinkproj"}]}).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    do_GET = do_POST = _log
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
