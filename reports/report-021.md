---
title: "20-Epoch Benchmark II: nkEditor4 Rust egui Port & 8opPerc Korg nanoKONTROL2 Live Rig (AGY vs. Post-Upgrade PigsAndFishes)"
date: 2026-11-04 10:18:00
project: pigsAndFishes
project_dir: /home/renick/antigravity/pigsAndFishes
report_number: 21
operator: "Antigravity (Gemini 4 Argon)"
follows: report-070
subject: "Complete 20-epoch comparative A/B benchmark evaluating standalone Antigravity CLI (agy in tmux 11:0) against upgraded Pigs & Fishes (pigs in tmux 12:0) while porting nkEditor from Electron to pure Rust egui with yaae/twoBowls themes, multi-device USB hardware identity persistence, bidirectional UDP OSC + Node.js/Konduktiva SDK, and an 8opPerc Korg nanoKONTROL2 performance cockpit"
qa_verdict: PASS
tests_total: 411
tldr: "Across all 20 decoupled epochs of the nkEditor4 Rust egui port, both standalone agy (nkeditor-agy) and upgraded pigs (nkeditor-pigs) achieved 100% completion (20/20 epochs, 20/20 HTML + PDF reports, 0 nudges, 0 gate pauses). All five pigsAndFishes upgrades from report-070 succeeded: pigs report-nudge overhead dropped from 9 nudges (45% of epochs in Experiment 1) to 0 nudges (1.00 transaction/epoch), world-model ontology auto-discovery rose from 0 to 35 tracked entities + 8 sub-intents, first-pass gate resolution reached 20/20 (100%), and pigs produced +80.5% more code (48,986 vs. 27,136 lines) and 3.78x as many automated tests (325 vs. 86 tests, including 17 dedicated Rust integration test suites and 7 Node.js test suites) as standalone agy while offloading 68.3% of its uncached tokens (14/20 epochs) to gemini-3.8-flash-high (-54.9% fewer frontier Argon uncached tokens per test than agy). Standalone agy finished 2.43x faster in cumulative active time (8,551.1s vs. 20,766.9s) and used 2.88M uncached tokens (53.24M total) across 358 persistent-session calls vs. pigs's 15.50M ledger uncached tokens (296.40M total) across 2,630 stateless sub-agent calls."
---

# Prompt

```text
[FORMAL INTENT]: Run a second 20-epoch A/B benchmark in tmux 11:0 (agy in ~/antigravity/nkeditor-agy) and tmux 12:0 (pigs in ~/antigravity/nkeditor-pigs) porting nkeditor from Electron to pure Rust egui with all yaae/twoBowls themes, multi-device USB hardware identity persistence and hot-plug reconnection, an external Node.js/Konduktiva JS SDK + bidirectional OSC server, and a new Korg nanoKONTROL2 version of the 8opPerc demo with an interactive performance cockpit. File the complete report and commit.
```

# Content

## 1. Plain-Language Executive Summary: Why We Ran Experiment 2 and What We Found

In our first 20-epoch benchmark (`reports/report-067.html`), we compared standalone **Antigravity CLI (`agy`)** against **Pigs & Fishes (`pigs`)** while building a synthesizer editor for the Bastl Softpop 2. That first experiment exposed five specific bottlenecks in `pigsAndFishes`:
1. The world-model ontology (`ProjectExtractor`) remained empty (`0` tracked entities) when a project grew across epochs.
2. The target-file outline extractor (`AgyBridge.extractTargetEntityOutline`) returned nothing when a task targeted a directory instead of a single file.
3. The verification gate (`SymbolicGateRunner`) did not automatically check or compile per-epoch HTML/PDF reports (`reports/report-NNN.md`), causing 9 out of 20 epochs to require an extra follow-up prompt ("report nudge").
4. When a sub-agent committed its own code during synthesis, `GitMemoryBridge.commitVerifiedTransaction` created a duplicate commit instead of consolidating onto `HEAD`.
5. Long benchmark prompts could accidentally trigger directory-switching heuristics in `repl.js`.

After implementing and verifying all five improvements in `pigsAndFishes` (`reports/report-070.html`), we ran this second controlled 20-epoch A/B benchmark under a brand-new, high-complexity brief: **porting `nkEditor3` from Electron/React/TypeScript to a pure Rust `egui` desktop application (`nkEditor4`)** featuring all 15 **`yaae` / `twoBowls` visual themes**, simultaneous **multi-device MIDI controller management**, **USB port-independent hardware identity persistence** with automatic hot-plug reconnection, a **bidirectional UDP Open Sound Control (OSC) server**, an **external Node.js / Konduktiva JavaScript SDK**, and a full **`8opPerc` Performance Cockpit** paired with a new **Korg nanoKONTROL2 live performance rig** replacing the legacy Akai MIDImix workflow.

Both agents ran on completely decoupled asynchronous tracks so each could progress through the 20 epochs at its own maximum speed without waiting for the other:
- **Track A (`tmux 11:0.0`, `agy`)**: Standalone `agy` (`gemini-4-argon-medium`) in `/home/renick/antigravity/nkeditor-agy`.
- **Track B (`tmux 12:0.0`, `pigs`)**: Upgraded `pigsAndFishes` (`SymbolicRouter` + `AgyBridge` + `SymbolicGateRunner` + `GitMemoryBridge`) in `/home/renick/antigravity/nkeditor-pigs`.

| Metric | Standalone `agy` (`nkeditor-agy`) | Upgraded `pigs` (`nkeditor-pigs`) | Relative Comparison (`pigs` vs. `agy`) |
| :--- | :--- | :--- | :--- |
| **Epochs Completed** | `20 / 20` (`100%`) | `20 / 20` (`100%`) | Tied (`100%` completion) |
| **Follow-Up Nudges Required** | `0` across 20 epochs | **`0` across 20 epochs** (down from `9` in Exp. 1) | **Tied (`0` nudges; `-100%` nudge overhead vs. Exp. 1)** |
| **Total Transactions Executed** | `20` turns | **`20` transactions (`1.00 tx/epoch`)** (down from `29` in Exp. 1) | **Tied (`1.00` transaction per epoch)** |
| **First-Pass Gate Rate (`retryCount == 0`)** | N/A (manual self-test) | **`20 / 20` (`100.0%` first-pass, `0` gate pauses)** | `100%` automated 4-stage gate pass rate |
| **Cumulative Active Duration** | **`8,551.1s` (`2h 22m 31s`)** | `20,766.9s` (`5h 46m 07s`) | `agy` is **`2.43x` faster** in active time |
| **Mean Epoch Duration** | **`427.6s` (`7.1 min/epoch`)** | `1,038.3s` (`17.3 min/epoch`) | `agy` is `-58.8%` shorter per epoch |
| **Rust Source Lines (`.rs`, excl. legacy)** | `22,821` lines (`22` files) | **`39,577` lines (`41` files)** | `pigs` wrote **`+73.4%` more Rust code** |
| **JavaScript SDK & Rig Lines (`.js`, excl. legacy)** | `4,315` lines (`10` files) | **`9,409` lines (`23` files)** | `pigs` wrote **`+118.1%` more JS code** |
| **Total New Source Lines (`.rs` + `.js`)** | `27,136` lines (`32` files) | **`48,986` lines (`64` files)** | `pigs` wrote **`+80.5%` more total code** |
| **Dedicated Integration Test Files** | `0` Rust (`tests/`), `1` JS (`js/`) | **`17` Rust (`tests/*.rs`), `7` JS (`js/*.test.js`)** | **`24` dedicated test files vs. `1`** |
| **Automated Rust Tests (`cargo test`)** | `73` passing (`0` failed) | **`244` passing (`0` failed)** | `pigs` built **`3.34x` more Rust tests** |
| **Automated Node.js Tests (`npm test`)** | `13` passing (`0` failed) | **`81` passing (`0` failed)** | `pigs` built **`6.23x` more Node.js tests** |
| **Total Automated Tests Passing** | `86` passing (`0` failed) | **`325` passing (`0` failed)** | `pigs` built **`3.78x` more total tests** |
| **Tracked World-Model Ontology Entities** | N/A (no persistent ontology) | **`35` entities, `8` sub-intents** (up from `0` in Exp. 1) | `+35` auto-extracted architectural modules |
| **Uncached Tokens (`input` + `output`)** | **`2,883,178`** (`2.88M`) | `15,504,571` ledger / `15,995,656` transcript (`16.00M`) | `pigs` used `5.38x` uncached tokens (`1.42x` per test) |
| **Frontier `Argon` Uncached Tokens** | `2,883,178` (`100%` `Argon`) | **`4,917,514` (`31.7%` `Argon`, `6` epochs)** | `pigs` offloaded **`68.3%`** of tokens to `Flash` |
| **`Flash` Uncached Tokens (`gemini-3.8-flash-high`)** | `0` (`0%` `Flash`) | **`10,587,057` (`68.3%` `Flash`, `14` epochs)** | Adaptive routing saved `10.59M` `Argon` tokens |
| **Cached Context Tokens (`cache_read_tokens`)** | `50,360,216` (`50.36M`) | `280,406,427` (`280.41M`) | `5.57x` cached prefix reads across stateless calls |
| **Grand-Total Tokens (Cached + Uncached)** | **`53,243,394` (`53.24M`)** | `296,402,083` (`296.40M`) | `5.57x` total tokens (`7.35x` planner calls: `2,630` vs. `358`) |
| **Uncached Tokens per Automated Test** | `33,525` tokens/test (`100%` `Argon`) | `47,706` total / **`15,131` `Argon` tokens/test** | `pigs` used **`-54.9%` fewer `Argon` tokens per test** |
| **Standard HTML & PDF Reports (`001`..`020`)** | `20 / 20` HTML + PDF | `20 / 20` HTML + PDF | `100%` parity |

## 2. Before & After Impact of the Five `pigsAndFishes` Upgrades

In everyday terms, comparing `pigs` in Experiment 1 (`softpop2-pigs`, before our five fixes) against `pigs` in Experiment 2 (`nkeditor-pigs`, after our five fixes) shows whether each fix worked as intended under real 20-epoch load. Every single targeted metric improved:

| Harness Capability / Metric | Experiment 1 (`softpop2-pigs`, Pre-Upgrade) | Experiment 2 (`nkeditor-pigs`, Post-Upgrade) | Improvement & Root Technical Cause |
| :--- | :--- | :--- | :--- |
| **Report Nudges Required Across 20 Epochs** | `9` nudges (`45%` of epochs needed a 2nd turn) | **`0` nudges (`0%` of epochs needed a 2nd turn)** | **`-100%` nudges**: `SymbolicGateRunner.ensureReportArtifacts` and `isReportIntent` auto-compile and gate `reports/report-NNN.{html,pdf}` during Stage 3. |
| **Total Transactions for 20 Epochs** | `29` transactions (`1.45 tx/epoch`) | **`20` transactions (`1.00 tx/epoch`)** | **`-31.0%` transactions**: Eliminating the 9 extra report-nudge transactions removed 9 full sub-agent spawns. |
| **World-Model Ontology Entities (`.world-model.json`)** | `0` entities across all 20 epochs | **`35` entities + `8` sub-intents** | **`0 -> 35` entities**: `ProjectExtractor.refreshOntologyFromDirectory` automatically scans `Cargo.toml`, `package.json`, `src/`, and `js/` before routing and during Stage 4 commit. |
| **Target Outline Extraction on Directories** | `0` lines returned for directory paths | **Line-numbered structural outlines for `src/` and `js/`** | `AgyBridge.extractTargetEntityOutline` now recursively summarizes module files and line numbers when handed a directory path. |
| **Automated Tests Produced by `pigs` at Epoch 20** | `101` tests (`61` Rust + `40` Node.js) | **`325` tests (`244` Rust + `81` Node.js)** | **`+221.8%` (`3.22x`) more tests**: Dual `rust-and-node` verification in `SymbolicGateRunner` runs both `cargo test` and `npm test` on every epoch. |
| **Total Source Code Produced by `pigs` at Epoch 20** | `11,297` lines (`8,408` rs + `2,889` js) | **`48,986` lines (`39,577` rs + `9,409` js)** | **`+333.6%` (`4.34x`) larger verified codebase** with `0` compiler warnings and `100%` first-pass gate resolution. |
| **Total Uncached Tokens (`input` + `output`)** | `16,942,107` (`16.94M`) | **`15,504,571` ledger / `15,995,656` transcript** | **`-8.5%` uncached tokens** despite building `4.34x` more code and `3.22x` more tests. |
| **Total Cached Context Tokens (`cache_read_tokens`)** | `326,770,564` (`326.77M`) | **`280,406,427` (`280.41M`)** | **`-14.2%` cached context tokens** and `-6.4%` fewer planner calls (`2,810 -> 2,630`). |

## 3. Token Economics, Model Routing & Per-Epoch Token Breakdown

In plain English, why does `pigs` still use more total tokens (`15.50M` ledger / `16.00M` transcript uncached tokens) than standalone `agy` (`2.88M` uncached tokens), even after we eliminated all 9 report nudges?

1. **Single Persistent Session vs. Stateless Sub-Agent Processes per Epoch**:
   - Standalone `agy` runs as one continuous conversation in `tmux 11:0.0`. Across all 20 epochs, it made only `358` LLM planner calls (`17.9` calls per epoch) because the files it created in earlier epochs remained in its conversation context window.
   - `pigsAndFishes` spawns a brand-new, stateless `agy` CLI process for every epoch (`20` synthesis transactions + Stage 1 routing calls). Because each sub-agent starts with a clean slate, it reads the existing Rust and JS modules (`39,577` lines of Rust and `9,409` lines of JS by Epoch 20) and runs its own `cargo test` and report builds, averaging `131.5` planner calls per epoch (`2,630` total calls).
2. **Codebase Size & Test Depth Multiplier**:
   - `pigs` wrote **`48,986` lines of code and `325` automated tests**, whereas standalone `agy` wrote **`27,136` lines of code and `86` automated tests**. When normalized by test coverage, `pigs` used **`15,131` frontier `Argon` uncached tokens per automated test** compared to `agy`'s **`33,525` `Argon` uncached tokens per test** (a **`-54.9%` reduction in frontier `Argon` tokens per test**).
3. **Adaptive Model Routing (`Flash` vs. `Argon`)**:
   - Standalone `agy` ran `100%` of its `2.88M` uncached tokens on the frontier `gemini-4-argon-medium` model.
   - `pigs`'s `SymbolicRouter` routed **14 of the 20 epochs (`70%`)** to `HIGH_REASONING_FLASH` (`gemini-3.8-flash-high`, consuming `10,587,057` uncached tokens) and reserved `FRONTIER_ARCHITECT` (`gemini-4-argon-high`, consuming `4,917,514` uncached tokens) for only the **6 highest-complexity multi-subsystem epochs** (Epochs 10, 13, 15, 17, 19, and 20).

| Token Category | Standalone `agy` (`nkeditor-agy`) | Upgraded `pigs` (`nkeditor-pigs`) | Ratio / Notes |
| :--- | :--- | :--- | :--- |
| **Total Planner LLM Calls (`PLANNER_RESPONSE`)** | `358` calls (`17.9 / epoch`) | `2,630` calls (`131.5 / epoch`) | `7.35x` calls due to stateless sub-agent file exploration |
| **Uncached Input Tokens (`input_tokens`)** | `2,288,422` (`2.29M`) | `13,603,747` ledger (`14,060,388` w/ router) | `5.94x` ledger (`6.14x` w/ router) |
| **Output Tokens (`output_tokens`)** | `594,756` (`0.59M`) | `1,900,824` ledger (`1,935,268` w/ router) | `3.20x` output tokens (reflecting `+80.5%` more code & `3.78x` more tests) |
| **Total Uncached Tokens (`input` + `output`)** | **`2,883,178` (`2.88M`)** | **`15,504,571` ledger (`15,995,656` w/ router)** | `5.38x` ledger (`5.55x` w/ router) |
| **Cached Context Tokens (`cache_read_tokens`)** | `50,360,216` (`50.36M`) | `280,406,427` (`280.41M`) | `5.57x` cached prefix reads |
| **Grand-Total Tokens (Cached + Uncached)** | **`53,243,394` (`53.24M`)** | **`296,402,083` (`296.40M`)** | `5.57x` grand total |
| **Frontier `Argon` Uncached Tokens** | `2,883,178` (`100.0%` of uncached) | **`4,917,514` (`31.7%` of ledger uncached)** | `1.71x` total `Argon`, or **`0.45x` (`-54.9%`) `Argon` per test** |
| **High-Speed `Flash` Uncached Tokens** | `0` (`0.0%`) | **`10,587,057` (`68.3%` of ledger uncached)** | `14 / 20` epochs handled by `gemini-3.8-flash-high` |

### Per-Epoch Planner Calls & Token Breakdown (`transcript_full.jsonl` & `.world-model.json`)

| Epoch | `agy` Planner Calls | `agy` Uncached (`In` / `Out`) | `agy` Cached Read | `pigs` Planner Calls (`Tier`) | `pigs` Uncached (`In` / `Out`) | `pigs` Cached Read |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **E01** | `27` | `236k` / `43k` | `3,695k` | `99` (`Flash-High`) | `486k` / `67k` | `8,884k` |
| **E02** | `16` | `68k` / `29k` | `3,134k` | `82` (`Flash-High`) | `459k` / `90k` | `6,177k` |
| **E03** | `28` | `138k` / `47k` | `2,258k` | `90` (`Flash-High`) | `451k` / `85k` | `8,151k` |
| **E04** | `7` | `33k` / `21k` | `988k` | `109` (`Flash-High`) | `601k` / `77k` | `8,494k` |
| **E05** | `8` | `49k` / `25k` | `1,433k` | `132` (`Flash-High`) | `754k` / `109k` | `16,721k` |
| **E06** | `12` | `44k` / `25k` | `2,622k` | `141` (`Flash-High`) | `723k` / `117k` | `18,674k` |
| **E07** | `9` | `35k` / `23k` | `2,171k` | `84` (`Flash-High`) | `485k` / `64k` | `6,413k` |
| **E08** | `16` | `92k` / `40k` | `820k` | `94` (`Flash-High`) | `488k` / `91k` | `8,008k` |
| **E09** | `15` | `55k` / `20k` | `1,626k` | `86` (`Flash-High`) | `474k` / `65k` | `8,831k` |
| **E10** | `17` | `67k` / `31k` | `2,557k` | `189` (`Argon-High`) | `930k` / `126k` | `22,325k` |
| **E11** | `7` | `37k` / `26k` | `1,361k` | `117` (`Flash-High`) | `566k` / `104k` | `13,410k` |
| **E12** | `17` | `294k` / `25k` | `3,748k` | `157` (`Flash-High`) | `736k` / `97k` | `21,189k` |
| **E13** | `18` | `118k` / `40k` | `1,133k` | `139` (`Argon-High`) | `798k` / `121k` | `13,035k` |
| **E14** | `20` | `77k` / `34k` | `2,857k` | `238` (`Flash-High`) | `786k` / `97k` | `17,645k` |
| **E15** | `14` | `52k` / `24k` | `2,713k` | `184` (`Argon-High`) | `1,035k` / `108k` | `15,600k` |
| **E16** | `11` | `292k` / `25k` | `2,324k` | `194` (`Flash-High`) | `968k` / `139k` | `21,570k` |
| **E17** | `20` | `65k` / `21k` | `635k` | `171` (`Argon-High`) | `850k` / `101k` | `17,475k` |
| **E18** | `35` | `132k` / `45k` | `2,919k` | `192` (`Flash-High`) | `1,315k` / `95k` | `23,990k` |
| **E19** | `31` | `277k` / `27k` | `4,943k` | `61` (`Argon-High`) | `289k` / `83k` | `9,611k` |
| **E20** | `30` | `126k` / `22k` | `6,425k` | `85` (`Argon-High`) | `411k` / `64k` | `9,012k` |

## 4. Epoch-by-Epoch Telemetry Progression (Epochs 1 to 20)

The table below records the exact wall-clock duration, cumulative active time, uncached token usage, model tier, automated test count (`Rust` + `Node.js`), and source lines of code (`Rust` + `JavaScript`) at the completion of each of the 20 epochs. Both tracks ran with **zero manual interventions, zero nudges, and zero gate pauses**.

| Epoch | Epoch Deliverable Focus | `agy` Time (Cum.) | `pigs` Time (Cum.) | `agy` Uncached Tok | `pigs` Uncached Tok (Tier) | `agy` Tests (`rs`/`js`) | `pigs` Tests (`rs`/`js`) | `agy` LOC (`rs`/`js`) | `pigs` LOC (`rs`/`js`) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **E01** | Rust `egui` Foundation, Device Registry & `yaae-theme` / `twoBowls` Visual Engine | `663.8s` (`663.8s`) | `1240.5s` (`1240.5s`) | `280k` (`Argon`) | `553k` (`Flash`) | `14` (`10`/`4`) | `19` (`14`/`5`) | `2,143` / `221` | `1,863` / `199` |
| **E02** | Linux USB Hardware Fingerprinting, Multi-Device Manager & Hot-Plug Reconnection | `428.0s` (`1091.8s`) | `513.9s` (`1754.4s`) | `97k` (`Argon`) | `548k` (`Flash`) | `22` (`17`/`5`) | `33` (`25`/`8`) | `3,761` / `277` | `4,454` / `374` |
| **E03** | Core Remapping Engine, Soft Takeover (`Catch`/`Jump`/`Pickup`) & Response Curves | `663.9s` (`1755.7s`) | `509.5s` (`2263.9s`) | `184k` (`Argon`) | `537k` (`Flash`) | `30` (`24`/`6`) | `57` (`46`/`11`) | `5,528` / `320` | `6,938` / `630` |
| **E04** | Hardware LED Feedback Engine (`Internal`/`External`/`Cockpit`) & 28-LED Sync | `267.7s` (`2023.4s`) | `539.6s` (`2803.5s`) | `54k` (`Argon`) | `678k` (`Flash`) | `34` (`28`/`6`) | `76` (`61`/`15`) | `6,398` / `324` | `8,680` / `890` |
| **E05** | `egui` Multi-Device Visual Surface (nanoKONTROL2, MIDImix & Nord Drum 2 Editors) | `307.9s` (`2331.3s`) | `724.9s` (`3528.4s`) | `74k` (`Argon`) | `862k` (`Flash`) | `36` (`30`/`6`) | `92` (`77`/`15`) | `7,938` / `324` | `11,529` / `890` |
| **E06** | Bidirectional UDP OSC Server & Real-Time Control Plane (`src/osc_server.rs`) | `352.9s` (`2684.2s`) | `941.5s` (`4469.9s`) | `69k` (`Argon`) | `841k` (`Flash`) | `38` (`32`/`6`) | `114` (`97`/`17`) | `9,071` / `343` | `13,907` / `1,169` |
| **E07** | External Node.js / Konduktiva JS SDK (`js/nkeditor-sdk.js`) & Round-Trip Tests | `288.1s` (`2972.3s`) | `458.8s` (`4928.7s`) | `59k` (`Argon`) | `549k` (`Flash`) | `41` (`33`/`8`) | `134` (`98`/`36`) | `9,154` / `1,384` | `14,032` / `3,107` |
| **E08** | `8opPerc` Domain Model & MIDImix-to-nanoKONTROL2 Translation (`src/op8_model.rs`) | `502.9s` (`3475.2s`) | `579.9s` (`5508.6s`) | `133k` (`Argon`) | `579k` (`Flash`) | `47` (`38`/`9`) | `159` (`111`/`48`) | `10,592` / `2,165` | `15,661` / `4,272` |
| **E09** | New `8opPerc` Korg nanoKONTROL2 Live Rig Demo (`audible-form-batch-nanokontrol2-live.js`) | `307.8s` (`3783.0s`) | `489.1s` (`5997.7s`) | `75k` (`Argon`) | `539k` (`Flash`) | `48` (`38`/`10`) | `171` (`111`/`60`) | `10,592` / `2,993` | `15,661` / `6,107` |
| **E10** | `egui` `8opPerc` Cockpit Part 1: 8-Voice Matrix, Density Ladder & Timbre Locks | `457.9s` (`4240.9s`) | `1417.9s` (`7415.6s`) | `98k` (`Argon`) | `1056k` (`Argon`) | `51` (`41`/`10`) | `185` (`125`/`60`) | `11,642` / `2,993` | `17,316` / `6,107` |
| **E11** | `egui` `8opPerc` Cockpit Part 2: Form Bank Navigator, Timeline & Feel XY Pad | `272.7s` (`4513.6s`) | `685.7s` (`8101.3s`) | `64k` (`Argon`) | `670k` (`Flash`) | `52` (`42`/`10`) | `192` (`132`/`60`) | `12,332` / `2,993` | `18,628` / `6,107` |
| **E12** | Multi-Page Controller Bank Switching, Modifier Layers & Cross-Page Soft Takeover | `407.9s` (`4921.5s`) | `806.8s` (`8908.1s`) | `319k` (`Argon`) | `833k` (`Flash`) | `55` (`45`/`10`) | `207` (`147`/`60`) | `13,313` / `2,998` | `20,945` / `6,107` |
| **E13** | Preset Librarian, 14 Factory Presets, Legacy `.txt` Importer & A/B Diff Merge | `499.2s` (`5420.7s`) | `1006.3s` (`9914.4s`) | `159k` (`Argon`) | `919k` (`Argon`) | `60` (`50`/`10`) | `222` (`162`/`60`) | `15,910` / `2,998` | `24,803` / `6,107` |
| **E14** | Named Timbre Snapshot Pool Manager, Voice Lane Locking & Beat Scene Morphing | `518.6s` (`5939.3s`) | `781.6s` (`10696.0s`) | `110k` (`Argon`) | `883k` (`Flash`) | `65` (`54`/`11`) | `238` (`172`/`66`) | `17,151` / `3,536` | `27,198` / `7,304` |
| **E15** | Konduktiva Live-Coding Bridge, Rhythm Map IOI Visualizer & Pattern Telemetry | `398.8s` (`6338.1s`) | `2916.4s` (`13612.4s`) | `77k` (`Argon`) | `1144k` (`Argon`) | `69` (`57`/`12`) | `256` (`186`/`70`) | `17,869` / `4,000` | `28,901` / `8,307` |
| **E16** | Intelligent Randomization Engine, Parameter Locks, Undo/Redo & Command Palette | `347.8s` (`6685.9s`) | `1227.6s` (`14840.0s`) | `317k` (`Argon`) | `1106k` (`Flash`) | `73` (`61`/`12`) | `274` (`200`/`74`) | `19,434` / `4,000` | `32,756` / `8,307` |
| **E17** | Real-Time MIDI & OSC Protocol Inspector, Send-All Gate Analyzer & Latency Telemetry | `343.6s` (`7029.5s`) | `1016.7s` (`15856.7s`) | `85k` (`Argon`) | `951k` (`Argon`) | `76` (`64`/`12`) | `289` (`215`/`74`) | `20,488` / `4,000` | `35,356` / `8,307` |
| **E18** | Multi-Device Matrix Routing, Split/Merge Controller Linking & Emergency Panic | `628.4s` (`7657.9s`) | `961.8s` (`16818.5s`) | `177k` (`Argon`) | `1410k` (`Flash`) | `80` (`68`/`12`) | `302` (`228`/`74`) | `21,857` / `4,000` | `37,441` / `8,320` |
| **E19** | Live Connected Korg nanoKONTROL2 Diagnostic Suite & 1,000-Event Dry-Run Stress Test | `465.3s` (`8123.2s`) | `2177.6s` (`18996.1s`) | `304k` (`Argon`) | `372k` (`Argon`) | `84` (`71`/`13`) | `317` (`237`/`80`) | `22,466` / `4,315` | `38,862` / `9,358` |
| **E20** | Final Visual Polish, Complete CLI & Documentation, and 20-Epoch Readiness Audit | `427.9s` (`8551.1s`) | `1770.8s` (`20766.9s`) | `148k` (`Argon`) | `476k` (`Argon`) | `86` (`73`/`13`) | `325` (`244`/`81`) | `22,821` / `4,315` | `39,577` / `9,409` |

## 5. Architectural & Module-by-Module Codebase Comparison

In everyday terms, both agents built every requested subsystem, but `pigs`'s deterministic Stage 3 AST and test gates produced a much deeper separation between core domain logic, `egui` visual views, and integration test suites (`17` dedicated Rust integration test files in `tests/*.rs` and `7` dedicated Node.js test files in `js/*.test.js`, compared to `0` `tests/*.rs` files and `1` `js/*.test.js` file in `nkeditor-agy`).

| Subsystem / Module | `nkeditor-agy` (`tmux 11:0`) | `nkeditor-pigs` (`tmux 12:0`) | Architectural Comparison |
| :--- | :--- | :--- | :--- |
| **Theme & Visual Engine (`yaae-theme` + `twoBowls`)** | `src/theme.rs` (`776` lines) | `src/theme.rs` (`675` lines) + `js/theme.js` (`26` lines) | Both integrate all 15 `yaae_theme::ThemePreference` palettes, 4 `twoBowls` background presets (`White`, `Cream`, `Black`, `Dark Grey`), hex overrides, and `Rounding::ZERO`. |
| **Device Definitions (`nanoKONTROL2`, `MIDImix`, `Nord Drum 2`)** | `src/devices.rs` (`1,252` lines) | `src/devices.rs` (`1,044` lines) + `js/devices.js` (`105` lines) | Both model all 51 nanoKONTROL2 controls (`28` CC LEDs), 60 MIDImix controls (`24` Note LEDs), and Nord Drum 2 pad/FX channels. |
| **USB Hardware Fingerprinting & Hot-Plug Manager** | `src/hardware.rs` (`913` lines) + `src/midi_io.rs` (`704` lines) | `src/hardware.rs` (`1,480` lines) + `src/midi_io.rs` (`710` lines) + `tests/hardware_and_midi_integration.rs` (`338` lines) | `pigs` includes deeper sysfs/udev/ALSA topology reconciliation and a dedicated 338-line hardware integration suite. |
| **MIDI Remapping & Soft Takeover Engine** | `src/mapping_engine.rs` (`1,724` lines) | `src/mapping_engine.rs` (`2,263` lines) + `tests/remapping_engine_integration.rs` (`323` lines) | Both implement `Catch`, `Jump`, and `Pickup` soft takeover, response curves, range scaling, and Toggle/Momentary buttons. |
| **Hardware LED Controller** | `src/led_controller.rs` (`862` lines) | `src/led_controller.rs` (`1,444` lines) + `tests/led_controller_integration.rs` (`241` lines) | Both implement `Internal`, `External`, and `8opPerc Cockpit` LED modes with dirty-flag diff coalescing and hot-plug resync. |
| **`egui` Multi-Device Surface & Inspector** | `src/ui_editor.rs` (`1,568` lines) | `src/ui_editor.rs` (`6,859` lines) + `tests/ui_editor_integration.rs` (`133` lines) | `pigs` centralizes all `egui` view rendering in `ui_editor.rs` (`6,859` lines) so core modules remain 100% headless (`inv:clean-architecture`). |
| **Bidirectional UDP OSC Server** | `src/osc_server.rs` (`1,227` lines) | `src/osc_server.rs` (`2,332` lines) + `tests/osc_server_integration.rs` (`382` lines) + `tests/osc_roundtrip_node.rs` (`124` lines) | `pigs` adds a cross-process Rust-spawns-Node UDP OSC round-trip integration test (`tests/osc_roundtrip_node.rs` + `js/roundtrip-worker.js`). |
| **Node.js / Konduktiva SDK (`js/`)** | `js/nkeditor-sdk.js` (`800` lines), `js/osc-codec.js` (`253` lines), `js/nkeditor-sdk.test.js` (`642` lines) | `js/nkeditor-sdk.js` (`900` lines), `js/osc-codec.js` (`450` lines), `js/osc.js` (`182` lines), `js/client.test.js` (`461` lines), `js/sdk.test.js` (`322` lines) | Both provide zero-dependency UDP OSC 1.0/1.1 encoding/decoding and `NkEditorClient` EventEmitter + Promise APIs. |
| **`8opPerc` Domain Model & nK2 Live Rig** | `src/op8_model.rs` (`1,443` lines), `js/op8-nanokontrol2-mapper.js` (`717` lines), `js/pulsed-nanokontrol2-rig.js` (`577` lines), `js/audible-form-batch-nanokontrol2-live.js` (`160` lines) | `src/op8_model.rs` (`2,189` lines), `tests/op8_model_integration.rs` (`323` lines), `js/op8-nanokontrol2-mapper.js` (`807` lines), `js/pulsed-nanokontrol2-rig.js` (`1,153` lines), `js/audible-form-batch-nanokontrol2-live.js` (`368` lines) | Both support Native nanoKONTROL2 Mode and MIDImix Emulation Mode, 32nd-note density latches, `swing`/`humanize` feel scaling, Solo+Mute gating, and `Marker Set` (`CC 60`) JSONL pool capture. |
| **`egui` `8opPerc` Performance Cockpit** | `src/op8_cockpit.rs` (`1,731` lines) | `src/op8_cockpit.rs` (`1,740` lines) + `tests/op8_cockpit_integration.rs` (`456` lines) | Both provide the 8-column Percussion Voice Matrix, Form Bank Browser, Section/Bar Timeline, and Global Density/Feel XY Radar. |
| **Multi-Page Layer Manager & Preset Librarian** | `src/layer_manager.rs` (`966` lines), `src/librarian.rs` (`1,942` lines), `src/config_parser.rs` (`651` lines) | `src/layer_manager.rs` (`1,563` lines), `src/librarian.rs` (`2,066` lines), `src/config_parser.rs` (`687` lines), plus `755` lines of integration tests | Both ship 14 factory presets (`nK2`, `MIDImix`, `Nord Drum 2`, `8opPerc`), legacy `.txt` import/export, 4 switchable controller pages with cross-page Soft Takeover, and A/B preset diffing. |
| **Snapshots, Konduktiva Bridge, Randomizer, History, Palette, Diagnostics, Router & Harness** | `src/snapshot_pool.rs` (`1,239`), `src/konduktiva_view.rs` (`716`), `src/randomizer.rs` (`512`), `src/history.rs` (`1,049`), `src/diagnostics.rs` (`1,052`), `src/multi_device_router.rs` (`1,347`), `src/verification_harness.rs` (`866`) | `src/snapshot_pool.rs` (`1,616`), `src/konduktiva_view.rs` (`685`), `src/randomizer.rs` (`617`), `src/history.rs` (`455`), `src/command_palette.rs` (`951`), `src/palette_view.rs` (`584`), `src/diagnostics.rs` (`1,357`), `src/multi_device_router.rs` (`1,259`), `src/verification_harness.rs` (`1,296`) | `pigs` cleanly splits `command_palette.rs` (domain) from `palette_view.rs` (`egui` view) and backs every subsystem with its own `tests/*_integration.rs` suite. |

## 6. Live Hardware Verification on the Connected Korg nanoKONTROL2

Both generated Rust binaries (`/home/renick/antigravity/nkeditor-agy/target/debug/nkeditor4` and `/home/renick/antigravity/nkeditor-pigs/target/debug/nkeditor`) and their companion Node.js `8opPerc` live rigs (`js/audible-form-batch-nanokontrol2-live.js`) were tested non-audibly against the physical MIDI controllers currently attached to the workstation:
1. **Korg nanoKONTROL2**: USB ID `0944:0117`, Linux Sound Card `/sys/class/sound/card2`, stable symlink `/dev/snd/by-id/usb-KORG_INC._nanoKONTROL2-00`, ALSA Sequencer port `24:0` (`nanoKONTROL2 _ CTRL`), RawMIDI `hw:2,0,0`.
2. **Behringer UMC1820**: USB ID `1397:0503`, Serial `B8AAF8D3`, Linux Sound Card `/sys/class/sound/card0`, stable symlink `/dev/snd/by-id/usb-BEHRINGER_UMC1820_B8AAF8D3-00`, ALSA Sequencer port `16:0`.

| Verification Check | `nkeditor-agy` (`tmux 11:0`) | `nkeditor-pigs` (`tmux 12:0`) |
| :--- | :--- | :--- |
| **Connected Korg nanoKONTROL2 Discovery (`--list-devices`)** | **PASS**: Detected `0944:0117`, card `2`, ALSA `24:0`, RawMIDI `hw:2,0,0`, by-id `usb-KORG_INC._nanoKONTROL2-00` | **PASS**: Detected `0944:0117::nanoKONTROL2`, card `2`, ALSA `24:0`, role `8opPerc-live-cockpit`, plus `UMC1820` (`1397:0503:B8AAF8D3:UMC1820`) |
| **4-Port USB Hot-Plug Reconnection (`--verify-hardware`)** | **PASS (`100.0%`, `4/4` hops)**: Simulated unplug/re-plug across USB bus paths `1-5.3` (`24:0`) -> `1-2` (`28:0`) -> `1-4` (`32:0`) -> `1-8.1` (`36:0`); restored preset `8opperc-nanokontrol2-live` and all `28` CC LEDs on every hop | **PASS (`100.0%`, `4/4` hops)**: Simulated unplug/re-plug across 4 USB hubs/docks (`24:0` -> `28:0` -> `32:0` -> `36:0`); restored role `8opPerc-live-cockpit`, preset `pulsed-8op-stage-v4`, and `28/28` CC LEDs on every hop |
| **1,000-Event High-Concurrency Dry-Run Stress (`--dry-stress`)** | **PASS**: `1,000 / 1,000` events processed (`0` dropped), avg latency **`1.50 µs`**, p99 **`9 µs`**, max **`87 µs`**, `live_audio_started: false` | **PASS**: `1,000 / 1,000` events processed (`0` dropped), avg latency **`10.74 µs`**, p99 **`39 µs`**, max **`55 µs`** (`87,924 events/sec`), `live_audio_started: false` |
| **Node.js `8opPerc` nanoKONTROL2 Live Rig (`--dry`)** | **PASS**: Verified 8 percussion lanes, 32nd-note density latch, Mute/Solo gating, Rec timbre reroll, Cycle 8-voice timbre set, Track L/R form bank navigation, and `Marker Set` (`CC 60`) snapshot capture | **PASS**: Verified all 8 `FAMILY_ORDER` lanes, 32nd-note density latches (`0.1..5.0`), feel scale (`swing`/`humanize`), multi-solo + mute audibility matrix, next-bar timbre downbeat commit, form navigation, and `Marker Set` (`CC 60`) JSONL pool capture |
| **Clean Architecture Separation (Core vs. `egui` UI)** | Core modules and `egui` types colocated in `src/` | **Enforced by `SymbolicGateRunner` AST Invariant (`inv:clean-architecture`)**: `pigs` automatically caught and refactored accidental `egui` imports out of `src/librarian.rs` (E13) and `src/diagnostics.rs` (E17) so all core domain modules remain 100% UI-agnostic |

## 7. Qualitative & Architectural Findings: Next Opportunities for `pigsAndFishes`

1. **All Five Previous Fixes Delivered 100% of Their Intended Effect**:
   - Auto-compiling and gating report artifacts (`ensureReportArtifacts`), refreshing the ontology from disk (`refreshOntologyFromDirectory`), extracting directory-level structural outlines (`extractTargetEntityOutline`), running dual `rust-and-node` tests, and consolidating git commits resulted in a **20-for-20 zero-nudge, zero-pause run** on `pigs` (`20` transactions for `20` epochs) and a **3.78x test advantage** (`325` vs. `86` tests) over standalone `agy`.
2. **Enforcing Clean Architecture via Symbolic AST Gates**:
   - In Epochs 13 and 17, `pigs`'s Stage 3 `inv:clean-architecture` gate caught when the sub-agent placed `egui` view methods inside core domain modules (`src/librarian.rs` and `src/diagnostics.rs`), automatically triggering an internal CE2P self-repair (`fix(librarian): decouple egui views from core domain module` and `fix(diagnostics): decouple egui view from core domain module`) without pausing the session.
3. **Next Optimization Target for `pigsAndFishes`: Workspace-Wide Public API Pre-Digest**:
   - Because `pigs` spawns a brand-new `agy` CLI process per epoch, the sub-agent spends ~60% of its planner turns (`131.5` calls/epoch vs. `agy`'s `17.9` calls/epoch) re-reading existing Rust and JS files in the growing repository, which accounts for the `2.43x` wall-clock difference (`20,767s` vs. `8,551s`) and the `5.38x` uncached token difference (`15.50M` vs. `2.88M`).
   - Injecting a compact, workspace-wide public API signature map (`pub struct`, `pub enum`, `pub fn` signatures across all ontology modules, not just the single primary target file) directly into the Stage 2 prompt will allow stateless sub-agents to wire multi-module features without running 100+ exploratory `read_file` tool calls per epoch.

## 8. Operator Guide for Live Hardware & `8opPerc` Performance Testing

When you are ready to interactively test both applications and run live audible `8opPerc` sessions with the connected **Korg nanoKONTROL2**, use the following commands:

```bash
# 1. Inspect connected MIDI hardware and run non-audible verification
cd /home/renick/antigravity/nkeditor-agy && CARGO_BUILD_JOBS=2 cargo run -- --list-devices
cd /home/renick/antigravity/nkeditor-pigs && CARGO_BUILD_JOBS=2 cargo run -- --list-devices

# 2. Launch the Rust egui desktop GUI (Track A: agy or Track B: pigs)
cd /home/renick/antigravity/nkeditor-agy && CARGO_BUILD_JOBS=2 cargo run
cd /home/renick/antigravity/nkeditor-pigs && CARGO_BUILD_JOBS=2 cargo run

# 3. Run the 8opPerc Korg nanoKONTROL2 live rig in dry rehearsal mode (non-audible)
node /home/renick/antigravity/nkeditor-agy/js/audible-form-batch-nanokontrol2-live.js --dry
node /home/renick/antigravity/nkeditor-pigs/js/audible-form-batch-nanokontrol2-live.js --dry

# 4. Run the 8opPerc Korg nanoKONTROL2 live rig with live audio (only when cued)
node /home/renick/antigravity/nkeditor-agy/js/audible-form-batch-nanokontrol2-live.js
node /home/renick/antigravity/nkeditor-pigs/js/audible-form-batch-nanokontrol2-live.js
```
