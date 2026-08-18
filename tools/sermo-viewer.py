#!/usr/bin/env python3
"""Reads the vault and writes a page you can read it on.

Brett: "a simple local webpage that can read the DB and filter the lines based
on tag would be great" - which is the tool that decides whether a million
generated lines are worth having. A corpus you cannot READ is a corpus nobody
judges, and the whole quality question is a judging question.

Python and sqlite3 because both ship with macOS: no install, no server, no
dependency to keep working a year from now. Run it and open the page.

    python3 tools/sermo-viewer.py            # serve it, and open a browser
    python3 tools/sermo-viewer.py --snapshot # write a standalone file instead

SERVED BY DEFAULT, because the alternative confused the first person to use
it: a written-out page bakes its lines in, so the browser's refresh button
shows the file as it was and the vault looks frozen. Brett, after playing a
while: "there are still only 16 lines....why?" - there were thirty-one. Served,
every reload reads the database as it stands, and there is nothing to remember.

A browser cannot run a command from a file:// page, which is why the obvious
answer - a button that regenerates it - is not available. Serving is how a
button gets to mean anything.

`--snapshot` still writes the self-contained file, which is the right thing for
sending to somebody or keeping a record of what the corpus was on a given day.
"""

import html
import json
import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
VAULT = HERE / "logs" / "sermo.sqlite"
PAGE = HERE / "logs" / "sermo-lines.html"


def read(vault: Path):
    """Every line and its tags, oldest first."""
    if not vault.exists():
        return []
    db = sqlite3.connect(f"file:{vault}?mode=ro", uri=True)
    lines = db.execute("SELECT id, t, w, once FROM line").fetchall()
    tags: dict[int, list[str]] = {}
    for line, tag in db.execute("SELECT line, tag FROM line_tag"):
        if tag:
            tags.setdefault(line, []).append(tag)
    db.close()
    return [
        {"id": str(i), "t": t, "w": w, "once": bool(once), "tags": sorted(tags.get(i, []))}
        for (i, t, w, once) in lines
    ]


TEMPLATE = """<!doctype html>
<meta charset="utf-8">
<title>Sermo &mdash; %(count)d lines</title>
<style>
  :root { color-scheme: dark; }
  body { margin: 0; background: #14140f; color: #e8dcc0;
         font: 15px/1.55 Iowan Old Style, Palatino, Georgia, serif; }
  header { position: sticky; top: 0; background: #14140f; padding: 18px 26px 12px;
           border-bottom: 1px solid #4a3f27; }
  h1 { margin: 0 0 10px; font-size: 20px; letter-spacing: .14em; color: #c9a227;
       text-transform: uppercase; font-weight: 500; }
  input { width: 100%%; padding: 9px 12px; background: #1d1c15; color: #e8dcc0;
          border: 1px solid #4a3f27; font: inherit; }
  #tally { color: #8a7f63; font-size: 13px; margin-top: 8px; }
  #tags { padding: 10px 26px; border-bottom: 1px solid #2a2418; }
  .tag { display: inline-block; margin: 2px 5px 2px 0; padding: 2px 8px;
         border: 1px solid #4a3f27; border-radius: 11px; cursor: pointer;
         font-size: 12px; color: #b3a583; }
  .tag:hover { border-color: #c9a227; color: #e8dcc0; }
  .tag.on { background: #c9a227; color: #14140f; border-color: #c9a227; }
  ol { list-style: none; margin: 0; padding: 6px 26px 60px; }
  li { padding: 9px 0; border-bottom: 1px solid #221d13; }
  .said { font-size: 16px; }
  .under { margin-top: 3px; font-size: 12px; color: #8a7f63;
           font-family: ui-monospace, Menlo, monospace; }
  .none { padding: 40px 26px; color: #8a7f63; }
  button { margin-left: 12px; padding: 3px 12px; background: #1d1c15; color: #c9a227;
           border: 1px solid #4a3f27; border-radius: 3px; cursor: pointer;
           font: inherit; font-size: 12px; letter-spacing: .08em; }
  button:hover { border-color: #c9a227; color: #e8dcc0; }
</style>
<header>
  <h1>Sermo &mdash; the vault <button id="again">reload</button></h1>
  <input id="find" placeholder="search the words, or type a tag" autofocus>
  <div id="tally"></div>
</header>
<div id="tags"></div>
<ol id="list"></ol>
<div class="none" id="none" hidden>Nothing here says that.</div>
<script>
const LINES = %(lines)s;
const chosen = new Set();

const census = new Map();
for (const line of LINES) for (const tag of line.tags)
  census.set(tag, (census.get(tag) || 0) + 1);

const tagbar = document.getElementById('tags');
for (const [tag, n] of [...census].sort((a, b) => b[1] - a[1])) {
  const el = document.createElement('span');
  el.className = 'tag';
  el.textContent = tag + ' \\u00b7 ' + n;
  el.onclick = () => {
    chosen.has(tag) ? chosen.delete(tag) : chosen.add(tag);
    el.classList.toggle('on');
    draw();
  };
  tagbar.append(el);
}

// The whole page is re-fetched, because it is SERVED and the server reads the
// vault on every request - so this is genuinely current, not a redraw of what
// was already here. On a saved snapshot it simply reloads the file, which is
// the honest behavior for a file that cannot know anything new.
document.getElementById('again').onclick = () => location.reload();

const find = document.getElementById('find');
const list = document.getElementById('list');
const tally = document.getElementById('tally');
const none = document.getElementById('none');
find.oninput = draw;

function draw() {
  const words = find.value.trim().toLowerCase();
  // EVERY chosen tag must hold, which is the same rule the game plays by -
  // a viewer that filtered on "any of these" would show you a set the game
  // would never pick from.
  const shown = LINES.filter(line =>
    [...chosen].every(tag => line.tags.includes(tag)) &&
    (!words || line.t.toLowerCase().includes(words) ||
     line.tags.some(tag => tag.toLowerCase().includes(words))));

  list.replaceChildren(...shown.map(line => {
    const li = document.createElement('li');
    const said = document.createElement('div');
    said.className = 'said';
    said.textContent = line.t;
    const under = document.createElement('div');
    under.className = 'under';
    under.textContent = line.tags.join('  ') + (line.once ? '   [once]' : '');
    li.append(said, under);
    return li;
  }));
  none.hidden = shown.length > 0;
  tally.textContent = shown.length + ' of ' + LINES.length + ' lines'
    + (chosen.size ? '  \\u2014  ' + [...chosen].join(' + ') : '');
}
draw();
</script>
"""


def page() -> str:
    """The page, against the vault as it stands this second."""
    lines = read(VAULT)
    return TEMPLATE % {
        "count": len(lines),
        "lines": json.dumps(lines, ensure_ascii=False),
    }


def snapshot() -> int:
    lines = read(VAULT)
    PAGE.parent.mkdir(parents=True, exist_ok=True)
    PAGE.write_text(page(), encoding="utf-8")
    tags = {tag for line in lines for tag in line["tags"]}
    print(f"{len(lines)} lines under {len(tags)} tags -> {PAGE}")
    return 0


def serve(port: int = 8731, open_browser: bool = True) -> int:
    """Serves the page, reading the vault fresh on every request."""
    import http.server
    import webbrowser

    class Reader(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            body = page().encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            # Never cached: the whole point is that a reload is current.
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass  # a request a second while somebody clicks around is not news

    where = f"http://localhost:{port}/"
    print(f"the vault is on {where} - reload for the latest; ctrl-C to stop")
    if open_browser:
        webbrowser.open(where)
    try:
        http.server.HTTPServer(("127.0.0.1", port), Reader).serve_forever()
    except KeyboardInterrupt:
        print("\nstopped")
    return 0


def main() -> int:
    if "--snapshot" in sys.argv:
        return snapshot()
    port = 8731
    if "--port" in sys.argv:
        port = int(sys.argv[sys.argv.index("--port") + 1])
    # `--no-open` is for the launcher, which opens the browser itself once the
    # server is answering - otherwise the first load can beat it there.
    return serve(port, open_browser="--no-open" not in sys.argv)


if __name__ == "__main__":
    sys.exit(main())
