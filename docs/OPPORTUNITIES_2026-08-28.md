# OpenCade opportunity portfolio — 2026-08-28

> Status: original decision record. Opportunities 1–5 (all scores 20+) were implemented on
> 2026-08-28; their real-world outcome signals remain unmeasured until physical alpha sessions run.
>
> Scope: 20 opportunities not present in the previous Proof-of-Match implementation plan. The
> portfolio is intentionally wider than the recommendation; only the top three should be calibrated
> now, and only one strategic bet should receive focused execution.

Implementation landed in ADR 0006, migration 009, the `opencade_test` game/core fixture, controller
launch preflight, canonical post-match receipts, the evidence-gated per-game route policy, and the
privacy-thresholded `/api/v1/public/compatibility` aggregate plus static compatibility site.

## The one move

Build a **No-ROM Proof-of-Play Kit**: an original, deterministic, redistributable libretro test core
and harmless test content, packaged with the existing alpha doctor, invite flow, compatibility
fingerprints, paired-report verifier, and campaign summary.

This is the smallest structural move that removes the alpha's largest coordination burden. It lets
testers exercise the real RetroArch native-process/netplay boundary without sourcing arcade content,
agreeing on a ROM set, or debugging game-specific behavior first. It does **not** prove FBNeo game
compatibility; it separates “does OpenCade orchestrate physical native netplay?” from “does this
specific external core and content work?”

The public libretro API is documented as a lightweight, permissively usable interface, while
RetroArch netplay documents determinism, serialization, and identical core/content as core
compatibility constraints. The implementation must be original and independently reviewed against
the repository's Apache-2.0 dependency policy; RetroArch remains a user-supplied GPLv3 external
process. See the official [libretro core-development documentation](https://docs.libretro.com/development/cores/developing-cores/),
[RetroArch netplay documentation](https://docs.libretro.com/development/retroarch/netplay/), and
[Libretro license matrix](https://docs.libretro.com/development/licenses/).

## Quick opportunities scan

### Asset inventory

- **Product:** an executable Proof-of-Match control plane and experimental RetroArch Proof-of-Play
  boundary.
- **Content:** architecture, clean-room guardrails, five game definitions, alpha procedures, report
  templates, and a flat Windows campaign kit.
- **Audience/distribution:** a Discord community and GitHub contributors; repository evidence does
  not quantify either audience.
- **Technology:** authenticated rooms, one-use invites, synchronized launch, safe native process
  execution, direct UDP/STUN probing, capability-scoped relay/tunnel primitives, compatibility
  fingerprints, privacy-safe evidence, telemetry, and CI-built Windows artifacts.
- **Data:** structured activation blockers and match/failure reports, but no committed physical
  two-host evidence.
- **Revenue:** donations are possible through Buy Me a Coffee; no usage or conversion baseline makes
  pricing work premature.

### Top three combinations

#### 1. No-ROM Proof-of-Play Kit — 24/25

- **Combination:** native-process adapter × alpha kit × match evidence × an original deterministic
  libretro test core.
- **Tier:** T1, combinatorial.
- **Effort:** 3–5 focused weeks.
- **Expected impact:** remove three major test variables—ROM provenance, content mismatch, and
  game-specific behavior—and target a 3× increase in doctor-to-completed-test conversion.
- **First step:** write an ADR specifying the minimal core behavior, license boundary, deterministic
  state, two-player input visualization, content identity, and explicit non-claim about FBNeo.

#### 2. Public evidence-derived compatibility map — 22/25

- **Combination:** privacy-safe reports × campaign aggregator × GitHub Pages × community trust.
- **Tier:** T1, combinatorial.
- **Effort:** 1–2 weeks after the first physical evidence exists.
- **Expected impact:** turn every accepted test into reusable setup guidance, contributor priorities,
  and proof that claims are earned rather than advertised.
- **First step:** define a k-anonymous aggregate schema with no room, user, path, endpoint, or raw
  fingerprint disclosure.

#### 3. Invite-to-match receipt loop — 21/25

- **Combination:** one-use invites × synchronized launch × paired reports × Discord distribution.
- **Tier:** T1/T3, combinatorial channel leverage.
- **Effort:** 1–2 weeks after one physical Proof-of-Play pass.
- **Expected impact:** make each successful test recruit the next tester and reduce manual alpha
  coordination; target 25% of completed tests producing a second accepted invite.
- **First step:** design a privacy-safe receipt containing only game definition, route class,
  compatibility status, result, and an expiring invite—not identity or network data.

### Bottleneck flip

- **Current bottleneck:** physical Proof-of-Play requires two Windows hosts plus matching external
  emulator/core/content before the product's orchestration can be isolated and tested.
- **10× move if removed:** a tester can validate the native netplay seam without copyrighted game
  content, creating evidence that compounds into compatibility guidance and alpha recruitment.
- **How to remove it:** ship the original deterministic test core as a bounded conformance fixture,
  then run the existing 10-room campaign against it before expanding game coverage.

### Pricing test

Not applicable. Do not add subscriptions, hosted tiers, or paid matchmaking before repeated physical
play is demonstrated. The 90-day value test is positioning and adoption: 20 unique testers, 10
accepted physical sessions, at least 8 verified pairs, and at least 3 independent Windows hardware
profiles.

## Twenty new opportunities

Scores use the quick combination matrix: asset strength, connection clarity, sub-two-week effort,
impact, and defensibility, each from 1–5. A lower effort score does not mean an idea is bad; it means
it should not displace the keystone now. “90-day signal” is the first falsifiable result, not a
forecast.

|   # | Opportunity                                   | Tier  | Score | Solo scope | 90-day signal                                                                                           | Say no to                                                          |
| --: | --------------------------------------------- | ----- | ----: | ---------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
|   1 | **No-ROM Proof-of-Play Kit**                  | T1    | 24/25 | 3–5 weeks  | 8/10 verified physical test-core sessions                                                               | More emulator adapters before this campaign                        |
|   2 | **Public evidence-derived compatibility map** | T1    | 22/25 | 1–2 weeks  | Three hardware/network cohorts shown without small-cohort leakage                                       | Publishing raw reports or fingerprints                             |
|   3 | **Invite-to-match receipt loop**              | T1/T3 | 21/25 | 1–2 weeks  | 25% of verified sessions create one accepted follow-on invite                                           | General-purpose social feeds and chat                              |
|   4 | **Evidence-guided route policy**              | T1    | 20/25 | 2–4 weeks  | Route choice beats the fixed policy on a held-out campaign without false-positive playable routes       | “AI NAT detection” or automatic tunnel enablement without evidence |
|   5 | **Controller and player-slot preflight**      | T1    | 20/25 | 1–2 weeks  | Zero campaign failures caused by missing/wrong controller assignment                                    | Controller remapping as a full settings product                    |
|   6 | **One-command local alpha lab**               | T1    | 19/25 | 1–2 weeks  | A clean Windows host reaches a two-client local rehearsal in under 15 minutes                           | Production installer polish before physical validation             |
|   7 | **Compatibility fingerprint clinic**          | T1    | 19/25 | 1 week     | Mismatch diagnostics resolve 80% of preflight failures without exposing paths/hashes                    | A downloadable core/ROM catalog                                    |
|   8 | **Signed build provenance and SBOM**          | T2    | 19/25 | 1 week     | Every alpha binary and container has a verifiable CI provenance record                                  | Paying for code signing before alpha retention exists              |
|   9 | **Alpha blocker-to-issue bridge**             | T1    | 18/25 | 1–2 weeks  | Repeated, k-anonymous blocker clusters generate actionable maintainer summaries                         | Auto-opening public issues containing user evidence                |
|  10 | **Game-definition contribution doctor**       | T1    | 18/25 | 2 weeks    | Five independently submitted definitions pass schema/license review                                     | Bulk-importing proprietary manifests                               |
|  11 | **Portable redacted support bundle**          | T1    | 18/25 | 1–2 weeks  | Median alpha triage requires no follow-up request for local logs                                        | Collecting arbitrary logs, paths, or machine identifiers           |
|  12 | **Alpha match-night orchestrator**            | T3    | 18/25 | 1 week     | Ten scheduled pairs produce at least eight complete evidence pairs                                      | Recurring events before one dry run succeeds                       |
|  13 | **Self-host readiness scorecard**             | T1    | 17/25 | 1–2 weeks  | Three external operators pass an automated deploy/readiness checklist                                   | A hosted commercial control plane now                              |
|  14 | **Relay sponsor receipt**                     | T1/T3 | 17/25 | 1–2 weeks  | Donations are attributed to measured relay uptime/capacity without paywalling play                      | Premium relay priority or selling user/network data                |
|  15 | **PT-BR alpha onboarding**                    | T3    | 17/25 | 1 week     | Five Brazilian testers complete the doctor without English assistance                                   | Broad localization before one region converts                      |
|  16 | **Accessibility-first readiness mode**        | T1    | 16/25 | 2–3 weeks  | Keyboard-only and screen-reader users complete the preflight script                                     | A visual redesign unrelated to task completion                     |
|  17 | **Community relay qualification kit**         | T6    | 16/25 | 3–5 weeks  | Two external nodes pass capability, abuse, latency, and revocation tests                                | An open federation before ticket abuse controls are proven         |
|  18 | **LAN-party appliance profile**               | T4    | 16/25 | 2–4 weeks  | One organizer runs five local matches from a documented private deployment                              | Accountless internet play or bypassing authentication              |
|  19 | **Spectator feasibility spike**               | T1    | 15/25 | 2 weeks    | A public-interface-only prototype proves a read-only participant can join without changing player state | Shipping spectator UX before playable native evidence              |
|  20 | **Tournament desk pilot**                     | T4    | 14/25 | 3–5 weeks  | One eight-player community bracket completes with auditable match receipts                              | Building brackets, rankings, and moderation as a platform          |

Relevant implementation surfaces are documented by their owners: GitHub supports
[artifact build-provenance and SBOM attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations),
and a custom Actions workflow can publish a
[static GitHub Pages site](https://docs.github.com/en/pages/getting-started-with-github-pages/creating-a-github-pages-site).
These capabilities make opportunities 2 and 8 inexpensive to test; they are not evidence of user
demand.

### Calibration rule

The table is a search space, not a backlog. Calibrate items 1–7, then apply an optimal-stopping
rule: commit to the first later candidate that clearly beats the current 24/25 leader. Do not run
20 workstreams. Items 17–20 are options gated by physical evidence, not near-term commitments.

## Strategic opportunity scan

### Grove: forces

- **Technology:** the repository already has almost all orchestration and evidence primitives; an
  original test core connects them to a real native netplay process at low marginal cost.
- **Customer behavior:** early testers need a trustworthy “does my setup work?” path before they
  need a broad game catalog. This is a hypothesis to measure, not a claimed market fact.
- **Complementors:** RetroArch exposes public netplay behavior, including TCP hosting and relay
  operation, but identical deterministic core/content remains required. OpenCade can make those
  variables controlled for conformance testing.
- **Distribution:** GitHub artifacts and Discord can distribute one identical fixture without a
  proprietary download service.
- **Regulation/licensing:** a deliberately original, non-game fixture avoids distributing ROMs and
  separates the external GPLv3 process from Apache-2.0 OpenCade code. A license review is still a
  release gate.
- **Timing window:** open but not demonstrated to be closing. Urgency comes from project sequencing,
  not a fabricated market deadline.

### Thiel: the secret

Most emulator-netplay projects treat game breadth as proof of product value. The contrarian truth is
that, before breadth, a **boring deterministic conformance fixture** is more valuable: it makes
network, orchestration, process, controller, and evidence failures independently diagnosable.

### Yeo: the keystone move

Original deterministic libretro fixture
→ identical legal test inputs on every machine
→ lower alpha setup variance
→ more completed physical sessions
→ trustworthy route/process evidence
→ public aggregate compatibility guidance
→ more testers and contributors
→ safer expansion to FBNeo games, tunnels, spectators, and community relays.

This is one reusable system fixture, not a campaign of unrelated features.

### Munger/Klein: pre-mortem hard gate

Assume it is February 2027 and the bet failed completely:

1. **The fixture passes but proves nothing about real games.** Mitigation: state the non-claim in the
   UI/report and require a separate FBNeo compatibility campaign.
2. **The core becomes a new emulator project.** Mitigation: freeze scope to deterministic video,
   two controller states, serialization, checksums, and clean exit; reject audio polish/gameplay.
3. **License boundaries are ambiguous.** Mitigation: independently review the header/source license,
   keep RetroArch external and user-supplied, generate an SBOM, and block release on uncertainty.
4. **Testers still cannot coordinate two machines.** Mitigation: pair the fixture with one expiring
   invite, synchronized launch, a local rehearsal, and a single match-night dry run.
5. **Evidence collection leaks or overclaims.** Mitigation: reuse the closed report schema, require
   paired verification, suppress small cohorts, and publish aggregates only.

Affordable loss: cap the experiment at 5 weeks, 120 focused hours, and US$100 of infrastructure. If
the fixture cannot reach a real two-host session inside that budget, stop rather than expanding it.

### Jobs: category definition

> OpenCade is the only clean-room arcade-netplay project with a redistributable, deterministic
> Proof-of-Play conformance kit and evidence-derived compatibility claims.

This makes “number of supported games” irrelevant during the alpha. Say **no** to friends, chat,
rankings, replays, tournaments, additional adapters, and automatic internet tunnel routing until the
conformance campaign passes.

### Naval: leverage audit

- **Permissionless:** original code, CI artifacts, documentation, and community distribution.
- **Specific knowledge:** Rust/Tauri security, native process orchestration, networking, evidence
  design, and clean-room discipline are already encoded in the repository.
- **Wealth/compounding asset:** this is an open-source trust and testing asset rather than immediate
  revenue. Each adapter, route, and release can reuse it.
- **Long-term game:** contributors can submit verified compatibility evidence repeatedly; the value
  grows without storing private player data.

### Bezos: regret test

The greater regret is adding visible product breadth, then discovering the physical native seam is
unreliable and impossible to diagnose. The five-week fixture is reversible; months of feature work
on an unproven seam is not. A day-one entrant would control the test variables before promising a
catalog.

### Quantitative sizing

Revenue TAM/SAM/SOM is not supportable from repository evidence and must not be invented. Use a
bottom-up validation funnel instead:

- **TAM proxy:** everyone reachable through the current Discord/GitHub channels who has two Windows
  hosts or can pair with another tester; count unknown until recruitment starts.
- **SAM proxy:** 20 consented testers in one 90-day alpha cohort.
- **SOM proxy:** 10 physical sessions, 8 verified report pairs, and 3 distinct hardware profiles.
- **Success probability:** intentionally unestimated until a five-pair recruitment dry run provides
  a base rate.
- **Payoff proxy:** physical Proof-of-Play evidence plus reuse in every future adapter/route test.
- **Failure cost:** at most 120 hours and US$100.
- **Optionality:** compatibility map, route policy, contributor kit, community relays, spectator
  qualification, and later game campaigns.

### Moat assessment

- **Evidence/process moat:** medium durability, starts with the first verified campaigns and grows
  through consistent privacy-safe reports.
- **Distribution/trust moat:** medium durability if claims remain narrower and more auditable than
  alternatives.
- **Technical moat:** low by itself; the fixture is copyable. The compound of fixture + verifier +
  compatibility corpus + clean-room reputation is harder to copy.
- **Copy risk:** high for the fixture alone (weeks), medium for the evidence flywheel (months).

### Opportunity score

| Dimension               |     Score | Reason                                                              |
| ----------------------- | --------: | ------------------------------------------------------------------- |
| 10X force               |       4/5 | Connects implemented systems and removes several alpha variables    |
| Secret quality          |       4/5 | Conformance before catalog breadth is specific and falsifiable      |
| Keystone leverage       |       5/5 | Produces at least six downstream effects                            |
| Lollapalooza            |       4/5 | Alpha kit, native adapter, reports, CI, and community channel align |
| Category ownership      |       4/5 | Evidence-first clean-room conformance is distinctive if shipped     |
| Permissionless leverage |       5/5 | Code, CI, docs, and community; no proprietary content required      |
| Regret asymmetry        |       4/5 | Cheap to bound now and expensive to discover the seam late          |
| Timing window           |       3/5 | Sequencing is urgent; external market closure is unproven           |
| **Total**               | **33/40** | Strong asymmetric bet; begin this week after the hard gates         |

### Four-week execution bridge

1. **Week 1 — ADR and spike:** specify the minimal core, compile a blank original core against the
   public API, complete license review, and demonstrate deterministic local serialization.
2. **Week 2 — observable two-player fixture:** render frame number and both controller states,
   checksum serialized state, and make core/content identity reproducible.
3. **Week 3 — alpha integration:** add doctor checks and safe adapter configuration; emit existing
   compatibility and match evidence without expanding the report schema unnecessarily.
4. **Week 4 — physical campaign:** run one local rehearsal, then five paired sessions. If they pass,
   continue to the 10-session gate and only then start the public compatibility aggregate.

WOOP:

- **Wish:** make physical native netplay independently reproducible.
- **Outcome:** a new tester reaches a paired, verified session without sourcing a ROM.
- **Obstacle:** the fixture expands into a game/core project instead of remaining a test instrument.
- **Plan:** if a proposed feature does not isolate a compatibility, input, serialization, or lifecycle
  failure, reject it from the fixture.

Implementation intention: **when the ADR and license review pass, reserve four focused weeks for the
fixture and pause all ungated M5–M7 breadth.**

### Kill criteria

Kill or re-scope the bet if any condition occurs:

- no legally clear original-core boundary after the week-one review;
- no deterministic serialize/unserialize loop by the end of week two;
- no two-host native session by the end of week four;
- fewer than 3 of the first 5 recruited pairs complete the doctor-to-report flow;
- the fixture requires modifying or redistributing RetroArch, FBNeo, ROMs, BIOS files, or other
  third-party binaries;
- work exceeds 120 hours before the first verified physical pair.

## Decision

**Ship this week:** the No-ROM Proof-of-Play ADR and blank deterministic-core spike.

**Build this quarter:** the no-ROM conformance kit, followed only after verified evidence by the
public compatibility map and invite-to-match receipt loop.

**Kill criteria:** stop at the first failed legal boundary, deterministic-state gate, four-week
physical gate, or 120-hour affordable-loss cap.
