#!/usr/bin/env python3
"""
MarketWatch Auth Proxy
A zero-dependency reverse proxy that adds a secure password barrier in front of MarketWatch (localhost:3000).
Requires NO changes to MarketWatch or the UI.
"""

import sys
import os
import secrets
import urllib.request
import urllib.error
import urllib.parse
from http.server import HTTPServer, BaseHTTPRequestHandler
from http.cookies import SimpleCookie

TARGET_HOST = "http://127.0.0.1:3000"
PROXY_PORT = int(os.environ.get("PROXY_PORT", 3030))
PASSWORD = os.environ.get("MW_PASSWORD", "")

if not PASSWORD and len(sys.argv) > 1:
    PASSWORD = sys.argv[1]

if not PASSWORD:
    PASSWORD = secrets.token_urlsafe(12)

# Session token for authenticated users
SESSION_TOKEN = secrets.token_hex(24)

LOGIN_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>MarketWatch &mdash; Access Restricted</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
      background: #0b0f19;
      color: #e2e8f0;
      height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 16px;
    }
    .card {
      background: #111726;
      border: 1px solid #1e293b;
      border-radius: 12px;
      padding: 32px 28px;
      width: 380px;
      max-width: 100%;
      box-shadow: 0 20px 40px -15px rgba(0,0,0,0.7);
      text-align: center;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 44px;
      height: 44px;
      border-radius: 10px;
      background: #2563eb;
      color: #fff;
      font-family: 'JetBrains Mono', monospace;
      font-weight: 700;
      font-size: 16px;
      margin-bottom: 16px;
      box-shadow: 0 4px 12px rgba(37,99,235,0.35);
    }
    h1 {
      font-size: 18px;
      font-weight: 700;
      color: #f8fafc;
      margin-bottom: 6px;
    }
    p {
      font-size: 13px;
      color: #94a3b8;
      margin-bottom: 24px;
      line-height: 1.4;
    }
    .error-msg {
      background: rgba(220, 38, 38, 0.15);
      border: 1px solid rgba(220, 38, 38, 0.4);
      color: #fca5a5;
      font-size: 12px;
      font-weight: 600;
      padding: 8px 12px;
      border-radius: 6px;
      margin-bottom: 16px;
    }
    form {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    input {
      width: 100%;
      padding: 11px 14px;
      background: #0b0f19;
      border: 1px solid #334155;
      border-radius: 8px;
      color: #f8fafc;
      font-family: 'JetBrains Mono', monospace;
      font-size: 14px;
      outline: none;
      transition: border-color 0.15s ease;
    }
    input:focus {
      border-color: #3b82f6;
      box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.2);
    }
    button {
      padding: 11px;
      background: #2563eb;
      color: #fff;
      border: none;
      border-radius: 8px;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
      transition: background 0.15s ease;
    }
    button:hover { background: #1d4ed8; }
    .footer {
      margin-top: 24px;
      font-size: 11px;
      color: #64748b;
      font-family: 'JetBrains Mono', monospace;
    }
  </style>
</head>
<body>
  <div class="card">
    <div class="badge">MW</div>
    <h1>MarketWatch Terminal</h1>
    <p>Private session. Enter access passphrase to unlock.</p>
    __ERROR_MSG__
    <form method="POST" action="/_login">
      <input type="password" name="password" placeholder="Passphrase" autofocus required autocomplete="current-password" />
      <button type="submit">Unlock Terminal &rarr;</button>
    </form>
    <div class="footer">NSE/BSE Top-150 Analytics</div>
  </div>
</body>
</html>
"""

class AuthProxyHandler(BaseHTTPRequestHandler):
    def is_authenticated(self):
        cookie_header = self.headers.get("Cookie")
        if not cookie_header:
            return False
        cookie = SimpleCookie()
        try:
            cookie.load(cookie_header)
            if "mw_auth" in cookie and cookie["mw_auth"].value == SESSION_TOKEN:
                return True
        except Exception:
            return False
        return False

    def do_login_post(self):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len).decode("utf-8", errors="ignore")
        params = urllib.parse.parse_qs(body)
        submitted = params.get("password", [""])[0]

        if secrets.compare_digest(submitted, PASSWORD):
            self.send_response(302)
            self.send_header("Location", "/")
            self.send_header("Set-Cookie", f"mw_auth={SESSION_TOKEN}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400")
            self.end_headers()
        else:
            self.send_response(401)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            html = LOGIN_HTML.replace("__ERROR_MSG__", '<div class="error-msg">Incorrect passphrase. Access denied.</div>')
            self.wfile.write(html.encode("utf-8"))

    def serve_login(self, error=False):
        self.send_response(200 if not error else 401)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        err_div = '<div class="error-msg">Incorrect passphrase. Access denied.</div>' if error else ''
        html = LOGIN_HTML.replace("__ERROR_MSG__", err_div)
        self.wfile.write(html.encode("utf-8"))

    def forward_request(self):
        url = f"{TARGET_HOST}{self.path}"
        req_headers = {}
        for k, v in self.headers.items():
            if k.lower() not in ["host", "cookie"]:
                req_headers[k] = v

        body = None
        content_len = int(self.headers.get("Content-Length", 0))
        if content_len > 0:
            body = self.rfile.read(content_len)

        req = urllib.request.Request(url, data=body, headers=req_headers, method=self.command)

        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                self.send_response(resp.status)
                for k, v in resp.getheaders():
                    if k.lower() not in [
                        "transfer-encoding",
                        "content-encoding",
                        "content-length",
                        "connection",
                        "keep-alive",
                    ]:
                        self.send_header(k, v)
                self.end_headers()
                self.wfile.write(resp.read())
        except urllib.error.HTTPError as e:
            self.send_response(e.code)
            for k, v in e.headers.items():
                if k.lower() not in [
                    "transfer-encoding",
                    "content-encoding",
                    "content-length",
                    "connection",
                    "keep-alive",
                ]:
                    self.send_header(k, v)
            self.end_headers()
            self.wfile.write(e.read())
        except Exception as e:
            self.send_response(502)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(f'{{"error": "Backend gateway error: {e}"}}'.encode("utf-8"))

    def handle_auth(self):
        # Strip query strings before routing so /_login?x=1 still logs in
        # instead of being forwarded to the backend behind the auth gate.
        path_only = self.path.split("?", 1)[0]
        if path_only == "/_login" and self.command == "POST":
            self.do_login_post()
            return

        if not self.is_authenticated():
            if self.path.startswith("/api/"):
                self.send_response(401)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(b'{"error": "Unauthorized. Please authenticate."}')
            else:
                self.serve_login()
            return

        self.forward_request()

    def do_GET(self): self.handle_auth()
    def do_POST(self): self.handle_auth()
    def do_DELETE(self): self.handle_auth()
    def do_HEAD(self): self.handle_auth()
    def do_OPTIONS(self): self.handle_auth()
    def do_PUT(self): self.handle_auth()
    def do_PATCH(self): self.handle_auth()

    def log_message(self, format, *args):
        # Filter noisy log lines, keep important hits
        sys.stderr.write(f"[AUTH PROXY] {self.address_string()} - {format%args}\n")

if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", PROXY_PORT), AuthProxyHandler)
    print("=" * 60)
    print("  MarketWatch Password-Protected Auth Proxy")
    print(f"  Target:     {TARGET_HOST}")
    print(f"  Listening:  http://127.0.0.1:{PROXY_PORT}")
    # Never print the passphrase: it leaks into shell history, logs, and
    # process supervisors. It was provided via MW_PASSWORD/argv already.
    print("  Passphrase: [set via MW_PASSWORD or argv — not echoed]")
    print("=" * 60)
    sys.stdout.flush()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping proxy...")
