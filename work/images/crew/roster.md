# Breakwater's wardroom: draft roster

**The numbers now live in `data/crew.toml`**, which the game reads and rereads while it runs. That file wins wherever it and these tables differ.

Five posts, three officers each. Each officer brings 2–3 specializations, and each specialization has
three tiers. Portraits are in this folder; prompts are in `style.txt`, `subjects.txt` and
`subjects-alt.txt`.

## How officers grow

- **Getting them.** After a leg, a hand of leader cards may be dealt; better legs mean a likelier hand
  that leans rarer. A Common officer has 2 specializations at tier I. An Uncommon has 2, with the
  first at tier II. A Rare has 3 at tier I. One officer holds each post, and a new one replaces the old.
- **Exercise.** A specialization earns experience when it does its job (the XP column). Use a skill
  and it ranks up faster.
- **Battle points.** Each leg also pays officer points by stars, spent in the yard on any
  specialization. Skills with no clean way to credit use (marked *general*) rank up from these alone.
- Tier thresholds are still to be set. A starting guess is 100 experience for tier II and 250 for tier III.

## Specializations by post

### Gunnery
| Skill | I | II | III | XP from |
|---|---|---|---|---|
| **Gun Fury** (key) | 10 s at half reload, 60 s cooldown | 12 s, 50 s cooldown | 15 s, 40 s cooldown | damage dealt during a fury |
| **Spotting Corrections** | each salvo on the same ship −15% scatter (to −30%) | −20% (to −40%) | −25% (to −50%) | hits from corrected salvos |
| **Rate Plot** | −10% scatter against ships under way | −20% | −30% | hits on ships making over half speed |

### Signals
| Skill | I | II | III | XP from |
|---|---|---|---|---|
| **Radar Operator** | contacts beyond line of sight, out to 1.5 km | 2.5 km | the guns can lay on radar contacts | damage to ships first seen on radar |
| **Target Identification** | marks the most pressing threat | also shows its aim and reload | +15% damage against it | damage to the marked threat |
| **Range Boost** | all weapons +20% reach | +30% | +40% | hits beyond stock reach |
| **Fall of Shot** | incoming main-gun salvos mark where they'll land | every shell, plus torpedo tracks | marks show sooner | *general* |

### Engineering
| Skill | I | II | III | XP from |
|---|---|---|---|---|
| **Forced Draught** | +8% speed and acceleration | +14% | +20% | *general* |
| **Steam Held** | a cooked engine runs at 35% instead of stalling | 50% | 70% | seconds steamed on a cooked engine |
| **Smoke Screen** (key) | 20 s screen, 90 s cooldown | 30 s, 75 s | 40 s, 60 s | enemy rounds that miss through smoke |
| **Hard Over** | rudder +15% | +25% | +35% | *general* |

### Damage control (First Lieutenant)
| Skill | I | II | III | XP from |
|---|---|---|---|---|
| **Second Repair Crew** | a second party: twice the repair power | each party +25% | each party +50% | hull mended by the second party |
| **Drilled Party** | steady repairs +15%, party moves at once | +30% | +45% | hull mended by steady repairs |
| **Shoring** | knocked-out parts return at 25% | 40% | 60% | parts brought back |
| **Counterflooding** | flooding drag −25% | −40% | −60% | water pumped out |

### Liaison
| Skill | I | II | III | XP from |
|---|---|---|---|---|
| **Bomber Strike** (key) | once a round, a swarm of light bombers hits the nearest port facility | more bombers, and you pick the facility | twice a round | facility damage done by the bombers |
| **Recon Flight** | the chart shows every base at the start | and any hidden fleet | a spotter plane over the marked ship: −15% scatter on it | *general* |
| **Fleet Gunfire** (key) | one heavy salvo from over the horizon on the mark, once a round | twice a round | a heavier salvo | damage done by the salvos |
| **Fighter Cover** | a pair of fighters breaks up the first air raid each round | four fighters | every raid | planes downed by the fighters |

## The officers

| Post | Officer | Rarity | Specializations | Portrait |
|---|---|---|---|---|
| Gunnery | Lt. Cdr. Ilse Varenholt | Common | Spotting Corrections, Rate Plot | gunnery.png |
| Gunnery | Lt. Augustin Brask | Uncommon | **Gun Fury**, Spotting Corrections | gunnery-brask.png |
| Gunnery | Master Gunner Henrik Sollum | Rare | Gun Fury, Spotting Corrections, Rate Plot | gunnery-sollum.png |
| Signals | Sub-Lt. Pip Callender, "Sparks" | Common | Radar Operator, Range Boost | signals.png |
| Signals | Lt. Noor Haddad | Uncommon | **Target Identification**, Fall of Shot | signals-haddad.png |
| Signals | Dr. Leopold Wyck | Rare | Radar Operator, Target Identification, Range Boost | signals-wyck.png |
| Engineering | Chief Engineer Devika Rao | Common | Steam Held, Forced Draught | engineer.png |
| Engineering | Lt. Teodor Marsk | Uncommon | **Smoke Screen**, Hard Over | engineer-marsk.png |
| Engineering | Cdr. Gunhild Brenner | Rare | Forced Draught, Steam Held, Smoke Screen | engineer-brenner.png |
| Damage control | Lt. Cdr. Elias Abernathy | Common | Second Repair Crew, Shoring | first-lieutenant.png |
| Damage control | Lt. Marisol Dalisay | Uncommon | **Drilled Party**, Counterflooding | firstlt-dalisay.png |
| Damage control | Lt. Cdr. Tavita Fuimaono | Rare | Second Repair Crew, Drilled Party, Shoring | firstlt-fuimaono.png |
| Liaison | Lt. Kaede Morrow | Common | Bomber Strike, Recon Flight | liaison.png |
| Liaison | Cdr. Aurelian Vance | Uncommon | **Fleet Gunfire**, Recon Flight | liaison-vance.png |
| Liaison | Lt. Rafael Quintero | Rare | Bomber Strike, Fighter Cover, Recon Flight | liaison-quintero.png |

Bold marks an Uncommon officer's specialization that starts at tier II.
