---
name: recap
description: Recap a Shipvivor play session from the run logs that the game writes to work/runs (one <CODE>.log per patrol run, with the code shown in the window title), then talk over what happened and how it felt. Use when the user types /recap, optionally followed by a run code such as BZURBWF, "last 3", or what they want to talk about.
argument-hint: "[run code | last N] [what you want to talk about]"
---

# Recap

The user's request: $ARGUMENTS

The user has been playing and wants to talk the session over. The logs say what happened.
Only the user knows how it felt. Your job is to give them back the shape of the run and
ask about the moments that matter.

## 1. Find the runs

Logs live in the project's `work/runs`, newest last by mtime:

```sh
ls -t "${CLAUDE_SKILL_DIR}/../../../work/runs"/*.log | head -n 8
```

- A code in the arguments, in any case: read `<CODE>.log`. If none matches, list the
  recent codes with their `# started:` lines and ask which one they meant.
- "build N": every run whose `# build:` line is N. That is the number at the foot of the
  title screen, counted up by each release build.
- "last N": the N newest.
- Nothing: the newest run, plus every other run whose `# launched:` line matches it.
  Those were one sitting of the game. Say how many runs you found.
- No logs: say that every build logs runs (`cargo run`, `--release` too), from the
  moment Enter is pressed at the title, unless it was built with `--no-default-features`.
  Then stop.

## 2. Read them

Read each log whole. They run to a few hundred lines.

- `#` header lines: the code, when the run started, the build (`dev` for a debug build;
  logs from before 2026-11-25 have no build line), when the game was launched, and the
  seed (`SHIPVIVOR_SEED=<seed> cargo run` sails the same waters again).
- Every other line: wall clock (UTC), leg clock (m:ss sailed this leg, frozen in menus),
  an event tag, details.
- `pulse` sums up the last 15 s of sailing: helm and speed, hull, what was fired and hit,
  hits taken by section, gains, enemy rounds fired, and the nearest enemy. A `quiet`
  pulse means nothing happened.
- One-off events: `leg`, `ship`, `sunk`, `contact` (a cruiser or bigger put to sea),
  `spotted`, `harriers` (a formation of 2–5 took off), `steam` (the boss is raising
  steam, 30 s before she sails), `flagship` (the boss sailed), `torpedo`, `collide`,
  `heavy` (one blow of 6% of the hull or more), `gear`, `section`, `hull` (fell through
  75/50/25/10%), `peril`, `base`, `stores`, `star`, `clock` (the clock ran out: the leg
  goes on, a star down), `level`, `refit`, `pause`/`resume`, `capture`.
- The `leg` line gives the clock to beat, the goals, the boss and the points a star
  pays. A patrol ends only when every base is down and any boss that sailed is sunk,
  or she sinks. Logs from before 2026-11-25 also show `extract` and "time ran out".
- `leg-end` + `tally` close each leg. `run-end`, `legs` and `tally` close a won or lost
  run. `end` means it was given up or the game was closed. A log that ends with neither
  stops where the game did, so it probably crashed or was killed. Say so.
- Damage is hull points lost, exactly. Hits taken are counted from shell bursts on the
  hull, so two shells in one step count as one. Main battery hits are hits on ships.
  Shells into bases count as fired but not as hits.
- `capture` lines name screenshots the user took during the run. Each has a `.md` note
  beside it. Look at the ones near the moments you talk about.

Use the `tally` lines for totals. Add up pulses only for a leg that has no tally.

Then look for `<CODE>.feedback.toml` beside the log. It holds the scores the user gave in
the game: in the yard after a round (`[[round]]`, by `number` and `sea`) and on the report
at the end of the run (`[match]`, with `outcome`). Each metric (`challenge`,
`frustration`, `engagement`, `motivation`) runs from -3 to +3, 0 is neutral, and a metric
left out was not scored. A table's `screenshots` are the captures taken in that round. No
file means nothing was scored, or the option is off on the title's Options screen.

## 3. Recap

- Open with one line: how the run ended (won, lost, given up), on which leg, after how
  long at sea.
- Then a short paragraph per leg. Give its shape (a quiet opening, when the first contact
  and the first hit came, the hardest stretch, how it ended) and the two to four moments
  that mattered, with leg-clock times. Add the numbers that tell the story: main battery
  hits against rounds fired and how many bounced, damage taken and where, the lowest hull,
  stars and which goals were met, the refits chosen and the route taken.
- With several runs, finish with a line or two comparing them: damage taken per minute,
  accuracy, how far each got.
- Keep it under a page. Leave out what didn't happen.

## 4. Talk about how it felt

- If the user scored the rounds, start from their scores: read each round's numbers
  against what its leg shows (high frustration where the hull took a beating, low
  engagement over a long quiet stretch) and ask about the scores that stand out or
  change most from round to round. Don't ask again for what a score already says.
- Pick the two or three moments most likely to have felt good or bad. Look for a long
  quiet stretch, a burst of heavy hits, most hits bouncing, a near sinking, a fight that
  dragged on, or a refit that changed things. Ask about each in one short question.
- Take the user's answers at face value. Their experience is the judge, not the numbers.
- When something felt off, find the code that drives it: grep for the constant or system,
  read it, don't guess from file names. Link it with `file:line`, then recommend one
  change. Make changes only when asked.
- Other Claude sessions edit this tree at the same time. Re-read a file right before you
  edit it, use exact-match edits, and run rustfmt only on the files you touched.
