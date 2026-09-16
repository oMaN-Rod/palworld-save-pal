#!/usr/bin/env python3
"""Local stand-in for palstudio.app/install plus the GitHub release backend.

Serves three things on one port so the install one-liners can be exercised
end-to-end with zero infrastructure:

  /install            User-Agent sniffed, exactly like signal-broker's
                      install.ts: curl/wget -> /install.sh, PowerShell ->
                      /install.ps1, browsers -> the GitHub releases page.
  /install.sh|.ps1    The repo-root installer scripts.
  /api/repos/<owner>/<repo>/releases/latest
                      Fabricated release metadata: every file in
                      --release-dir becomes an asset of --tag, with
                      browser_download_urls pointing back at this server.
  /<owner>/<repo>/releases/download/<tag>/<asset>
                      The file itself, straight from --release-dir.

Typical use (see scripts/test-install-flow.sh):

  python3 scripts/install-mock-server.py --release-dir /tmp/fake-release \
      --tag v9.9.9-test --port 8917 &

  PALSTUDIO_API_BASE=http://127.0.0.1:8917/api \
  PALSTUDIO_DOWNLOAD_BASE=http://127.0.0.1:8917 \
      curl -fsSL http://127.0.0.1:8917/install | sh
"""

import argparse
import json
import re
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote

RELEASES_URL = "https://github.com/oMaN-Rod/palworld-save-pal/releases"

REPO_ROOT = Path(__file__).resolve().parent.parent

LATEST_RE = re.compile(r"^/api/repos/([^/]+/[^/]+)/releases/latest$")
DOWNLOAD_RE = re.compile(r"^/([^/]+/[^/]+)/releases/download/([^/]+)/([^/]+)$")


def install_target(user_agent):
    # Mirrors signal-broker/src/install.ts -- keep the two in step.
    ua = (user_agent or "").lower()
    if "powershell" in ua:
        return "ps1"
    if "curl" in ua or "wget" in ua:
        return "sh"
    if "mozilla" in ua:
        return "browser"
    if "windows" in ua:
        return "ps1"
    return "sh"


class Handler(BaseHTTPRequestHandler):
    release_dir: Path
    tag: str

    def log_message(self, fmt, *args):
        print(f"[mock] {self.address_string()} {fmt % args}", file=sys.stderr)

    def _redirect(self, location):
        self.send_response(302)
        self.send_header("Location", location)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def _bytes(self, body, content_type):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _file(self, path, content_type):
        try:
            body = path.read_bytes()
        except OSError:
            self.send_error(404, f"no such asset: {path.name}")
            return
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = unquote(self.path)

        if url in ("/install", "/install/"):
            target = install_target(self.headers.get("User-Agent"))
            if target == "browser":
                return self._redirect(RELEASES_URL)
            return self._redirect(f"/install.{target}")

        if url == "/install.sh":
            return self._file(REPO_ROOT / "install.sh", "text/x-shellscript")
        if url == "/install.ps1":
            return self._file(REPO_ROOT / "install.ps1", "text/plain")

        match = LATEST_RE.match(url)
        if match:
            assets = []
            for path in sorted(self.release_dir.iterdir()):
                if not path.is_file():
                    continue
                name = path.name
                repo = match.group(1)
                assets.append(
                    {
                        "name": name,
                        "browser_download_url": (
                            f"http://{self.headers.get('Host')}/{repo}/"
                            f"releases/download/{self.tag}/{name}"
                        ),
                    }
                )
            body = json.dumps({"tag_name": self.tag, "assets": assets}).encode()
            return self._bytes(body, "application/json")

        match = DOWNLOAD_RE.match(url)
        if match:
            name = Path(match.group(3)).name  # refuse any path traversal
            return self._file(self.release_dir / name, "application/octet-stream")

        self.send_error(404, f"unknown mock route: {url}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--release-dir", required=True, type=Path,
                        help="directory whose files become release assets")
    parser.add_argument("--tag", required=True, help="release tag to report, e.g. v9.9.9-test")
    parser.add_argument("--port", type=int, default=8917)
    parser.add_argument("--bind", default="127.0.0.1")
    args = parser.parse_args()

    Handler.release_dir = args.release_dir.resolve()
    Handler.tag = args.tag
    server = ThreadingHTTPServer((args.bind, args.port), Handler)
    print(f"[mock] serving install endpoint + release {args.tag} "
          f"from {Handler.release_dir} on http://{args.bind}:{args.port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()
