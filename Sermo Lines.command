#!/bin/bash
# Opens the vault in a browser: every line ChatGPT has written for this game,
# with its tags, searchable and filterable.
#
# Brett: "put a .command in the root of the Divus Factus folder to see the
# database" - one double-click, the same as launching the game.
#
# AND IT LEAVES NO TERMINAL BEHIND. A .command always opens Terminal, because
# that is what the extension means; what it does not have to do is STAY there.
# Brett: "it also boot up a terminal in the background can we have the boot
# silently?" So the server is detached with `nohup` and this script exits at
# once, which lets Terminal close its window on its own.
#
# It SERVES the page rather than writing one out, because a written page bakes
# its lines in and goes stale the moment the game writes another: the first
# time this was tried the vault held thirty-one lines and the page still said
# sixteen.
#
# Run it again later and it notices the server is already up, and just opens
# the browser at it. Nothing to install: python3 and sqlite3 ship with macOS.

cd "$(dirname "$0")" || exit 1

PORT=8731
HERE="http://localhost:$PORT/"

if [ ! -f logs/sermo.sqlite ]; then
    osascript -e 'display alert "No vault yet" message "The database appears the first time the game runs with the living voice on.

Launch the game, then Settings → SERMO → ChatGPT."' >/dev/null 2>&1
    exit 0
fi

# Already serving from an earlier double-click? Then just look at it.
if curl -s -o /dev/null --max-time 1 "$HERE"; then
    open "$HERE"
    exit 0
fi

# Detached, with its output thrown away: nothing is left holding this window.
mkdir -p logs
nohup python3 tools/sermo-viewer.py --port "$PORT" --no-open \
    >logs/sermo-viewer.log 2>&1 &
disown 2>/dev/null

# Wait for it to answer before opening the browser, or the first load 404s.
for _ in $(seq 1 40); do
    curl -s -o /dev/null --max-time 1 "$HERE" && break
    sleep 0.1
done

open "$HERE"
exit 0
