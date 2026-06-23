#!/usr/bin/env python3
"""Metal Archives Playwright Proxy — optional sidecar for youtui.

Bypasses Cloudflare on metal-archives.com using headless Firefox via Playwright.
Provides a local HTTP API at http://localhost:5000 that youtui's metal_api.rs
provider will try as a fallback when the metal-api.dev service is down.

Installation:
  pip install playwright beautifulsoup4
  playwright install firefox

Usage:
  python metal-archives-proxy.py

Then run youtui as normal. The metadata registry will auto-detect the proxy.
"""

import json, re, sys, time, signal, logging
from datetime import datetime
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs, quote
from pathlib import Path

try:
    from playwright.sync_api import sync_playwright
    from bs4 import BeautifulSoup
except ImportError:
    print("Missing dependencies. Install with: pip install playwright beautifulsoup4")
    print("Then: playwright install firefox")
    sys.exit(1)

PORT = 5000
CACHE = {}
CACHE_TTL = 15 * 86400  # 15 days

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
log = logging.getLogger("ma-proxy")

class MAHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        params = {k: v[0] for k, v in parse_qs(parsed.query).items()}
        path = parsed.path

        if path == "/search":
            result = search_albums(params.get("artist", ""), params.get("album", ""))
        elif path == "/album":
            result = get_album(params.get("url", ""))
        elif path == "/artist_info":
            result = get_artist_info(params.get("url", ""))
        elif path == "/ping":
            result = {"status": "ok"}
        else:
            result = {"error": "unknown path"}
        self._json(result)

    def _json(self, data):
        j = json.dumps(data, ensure_ascii=False).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(j)))
        self.end_headers()
        self.wfile.write(j)

def _cached(key, fetch_fn):
    now = time.time()
    if key in CACHE and now - CACHE[key][0] < CACHE_TTL:
        log.info("Cache hit: %s", key)
        return CACHE[key][1]
    data = fetch_fn()
    CACHE[key] = (now, data)
    return data

def _playwright_page():
    p = sync_playwright().start()
    browser = p.firefox.launch(headless=True)
    ctx = browser.new_context()
    page = ctx.new_page()
    page.route("**/*.{png,jpg,jpeg,gif,css,woff,woff2,ttf,svg}", lambda route: route.abort())
    return p, page

def search_albums(artist, album):
    cache_key = f"search:{artist}|{album}"
    def _search():
        url = f"https://www.metal-archives.com/search/ajax-advanced/searching/albums/?sEcho=1&iColumns=4&exactBandMatch=1&bandName={quote(artist)}&releaseTitle={quote(album)}"
        pw, page = _playwright_page()
        resp_data = {}
        def on_resp(resp):
            if "ajax-advanced/searching/albums" in resp.url and resp.status == 200:
                try: resp_data.update(resp.json())
                except: pass
        page.on("response", on_resp)
        page.goto(url, wait_until="domcontentloaded", timeout=30000)
        page.wait_for_timeout(2000)
        pw.stop()
        if "aaData" not in resp_data or not resp_data["aaData"]:
            return {"results": []}
        results = []
        for row in resp_data["aaData"]:
            artist_text = re.sub(r"<.*?>", "", row[0]).strip()
            m = re.search(r'href="([^"]+)"[^>]*>([^<]+)', row[1])
            album_url = m.group(1) if m else ""
            album_title = m.group(2) if m else ""
            date_raw = row[3]
            dm = re.search(r'<!--\s*(\d{4})', date_raw)
            year = dm.group(1) if dm else date_raw.strip()
            results.append({"artist": artist_text, "album": album_title, "url": album_url, "year": year})
        return {"results": results}
    return _cached(cache_key, _search)

def get_album(url):
    if not url: return {"error": "no url"}
    def _album():
        pw, page = _playwright_page()
        page.goto(url, wait_until="domcontentloaded", timeout=30000)
        html = page.content()
        pw.stop()
        soup = BeautifulSoup(html, "html.parser")
        def txt(sel):
            el = soup.select_one(sel)
            return el.text.strip() if el else ""
        def dt_text(label):
            dt = soup.find("dt", string=re.compile(label, re.I))
            return dt.find_next_sibling("dd").text.strip() if dt and dt.find_next_sibling("dd") else ""
        release_raw = dt_text("Release date:")
        year = re.search(r"\d{4}", release_raw)
        tracks = []
        for tr in soup.select("table.table_lyrics tr"):
            if "wrapWords" in str(tr):
                cols = tr.find_all("td")
                if len(cols) >= 4:
                    title = cols[1].text.strip()
                    length = cols[2].text.strip()
                    tracks.append({"title": title, "length": length})
        return {
            "album": txt("h1.album_name"),
            "artist": txt("h2.band_name a"),
            "year": year.group(0) if year else "",
            "metal_archives_type": dt_text("Type:"),
            "tracks": tracks,
        }
    return _cached(f"album:{url}", _album)

def get_artist_info(url):
    if not url: return {"error": "no url"}
    def _info():
        pw, page = _playwright_page()
        page.goto(url, wait_until="domcontentloaded", timeout=30000)
        html = page.content()
        pw.stop()
        soup = BeautifulSoup(html, "html.parser")
        def dt_text(label):
            dt = soup.find("dt", string=re.compile(label, re.I))
            return dt.find_next_sibling("dd").text.strip() if dt and dt.find_next_sibling("dd") else ""
        return {
            "genre": dt_text("Genre:"),
            "country": dt_text("Country of origin:"),
            "formation_year": dt_text("Formed in:"),
            "status": dt_text("Status:"),
        }
    return _cached(f"band:{url}", _info)

if __name__ == "__main__":
    log.info("Preloading Metal Archives session...")
    try:
        pw, page = _playwright_page()
        page.goto("https://www.metal-archives.com/", wait_until="domcontentloaded", timeout=15000)
        title = page.title()
        pw.stop()
        if "Just a moment" in title:
            log.warning("Cloudflare challenge detected. Proxy may not work.")
        else:
            log.info("Session preloaded successfully.")
    except Exception as e:
        log.warning("Preload failed: %s", e)

    server = HTTPServer(("0.0.0.0", PORT), MAHandler)
    log.info("Metal Archives proxy running on http://localhost:%d", PORT)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        log.info("Shutting down.")
        server.server_close()
