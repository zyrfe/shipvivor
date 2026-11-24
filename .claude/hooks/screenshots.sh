#!/bin/bash
# UserPromptSubmit hook: tell Claude about Shipvivor dev captures (the ` key in a debug
# build) taken since this session's previous prompt. Each capture is a PNG plus a .md
# note on the game state, in work/screenshots.
dir="${CLAUDE_PROJECT_DIR:-$PWD}/work/screenshots"
[ -d "$dir" ] || exit 0
session=$(jq -r '.session_id // empty')
[ -n "$session" ] || exit 0

seen="$dir/.seen"
mkdir -p "$seen"
marker="$seen/$session"
next="$marker.next"
touch "$next"
# The note is written the moment the key is pressed; the PNG follows a second later.
if [ -e "$marker" ]; then
  notes=$(find "$dir" -maxdepth 1 -name '*.md' -newer "$marker" | sort)
else
  # A session's first prompt only hears about the last 15 minutes.
  notes=$(find "$dir" -maxdepth 1 -name '*.md' -mmin -15 | sort)
fi
mv -f "$next" "$marker"
[ -n "$notes" ] || exit 0

count=$(printf '%s\n' "$notes" | wc -l | tr -d ' ')
echo "The user took $count Shipvivor dev capture(s) since their last message (oldest first)."
echo "Each PNG has a .md note beside it with the game state at that moment. If the message is"
echo "about what is on screen, Read the newest image and its note before answering."
printf '%s\n' "$notes" | tail -n 4 | while IFS= read -r note; do
  echo "- ${note%.md}.png (state: $note)"
done
[ "$count" -gt 4 ] && echo "- ...and $((count - 4)) earlier, in $dir"
exit 0
