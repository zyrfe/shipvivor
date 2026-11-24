---
name: screenshot
description: Look at the user's latest Shipvivor dev capture (the ` key in a debug build saves a PNG and a game-state note to work/screenshots) and discuss it with the user's comment in mind. Use when the user types /screenshot, optionally followed by what they see or want changed, e.g. "/screenshot the harbor looks like hell".
argument-hint: "[what you see, or what should change]"
---

# Screenshot

The user's comment: $ARGUMENTS

They pressed ` in the game and want to talk about what was on screen without describing it.
Look at the capture yourself, work out what they mean, and answer in terms of the game's
code.

## 1. Find the capture

Captures live in the project's `work/screenshots`. Each one is a `<UTC stamp>.png` with a
`<UTC stamp>.md` note beside it. The stamps sort by time, so the last one listed is the
newest:

```sh
find "${CLAUDE_SKILL_DIR}/../../../work/screenshots" -maxdepth 1 -name '*.png' | sort | tail -n 5
```

- Usually you only need the newest capture. Read more than one if the comment says so
  ("the last two", "before and after", "compare"), or if the newest note was written well
  after the one before it and the comment seems to be about the earlier screen.
- A `.png.partial` means that capture is still being encoded. Wait a second and look again.
- If there are no captures, say so in one line: press ` in a debug build (`cargo run`) and
  try again. Then stop.

## 2. Look before you answer

- Read the PNG. It is saved at the window's logical size, so its pixels match UI layout units.
- Read its `.md` note. That gives you the screen, helm, target, contacts and run seed. Trust
  the note over your reading of the image for state, and use the image for how things look.
- The user's eye is the judge of how things look, and the cartoony art is deliberate. Take
  the comment at face value and find what in the image they mean. Don't argue that it looks
  fine.

## 3. Tie it to the code

Name the part of the screen the comment is about, then find the code that draws or drives
it. Grep for the HUD label, component or system name, and read the code. Don't guess from
file names. Say what in the image comes from which code. If you can't tell which part they
mean, list the one or two most likely parts and ask.

## 4. Answer, and change things only when asked

- Open with what you see that matches the comment, in a sentence or two. Then give the cause
  in the code, with `file:line` links, and the change you recommend. Give one
  recommendation, not a survey.
- If the comment asks for a change ("make the X bigger", "fix the Y"), make it. If it is a
  reaction or a question ("the harbor looks like hell", "why is that red?"), diagnose and
  recommend, then offer to make the change.
- Other Claude sessions edit this tree at the same time, and there is no git. Re-read a file
  right before you edit it, use exact-match edits, run rustfmt only on the files you touched,
  and message any peer that has said it owns the file.
- After a visual change, say what to look for, and that pressing ` again lets you compare the
  new capture with this one.
- With no comment, describe what's on screen in a few lines and point out anything that looks
  broken: overlapping panels, clipped text, empty boxes where a glyph is missing, odd
  geometry.
