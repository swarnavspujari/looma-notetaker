# LLM Benchmarks — default local model decision

*Last run: 2026-07-13/14, on a Windows 11 laptop — RTX 3050 Laptop (4 GB VRAM), 32 GB RAM,
12 cores. All local models served by Ollama 0.x on this machine; latency numbers are
end-to-end (prompt → full response) on this hardware and will differ on other machines.*

This document grounds the "which model should `llm.ollama.model` default to?" decision.
It reports a full run of the repeatable harness (`src-tauri/tests/llm_bench.rs`) over the
app's four LLM tasks. **No model switch ships with this report** — it is evidence, not a
decision.

## 1. Verifying the external model claims

The external research suggested "Qwen3.5-4B" and "Gemma 4 E4B" as stronger small models.
Checked against the Ollama registry (ollama.com/library) on 2026-07-13, then confirmed by
actually pulling each model:

| Claim | Registry reality | Verdict |
|---|---|---|
| "Qwen3.5-4B" | `qwen3.5:4b` exists — 3.4 GB, 256K context, multimodal family, 15.5M pulls | **Real** |
| "Gemma 4 E4B" | `gemma4:e4b` exists — 9.6 GB, 128K context (gemma4 also ships 12b/26b/31b; the search page hides the e-tags) | **Real** |
| — (third candidate, own judgment) | `gemma3:4b` — 3.3 GB, 128K context, 38.5M pulls; the most-used current ~4B general model | Added |

Baseline: `llama3.1` (8B, 4.9 GB — today's default). Reference: `claude-sonnet-5` via the
existing AnthropicProvider (the app's default cloud model).

## 2. Methodology

- **Harness**: `src-tauri/tests/llm_bench.rs` (`#[ignore]`d test; run instructions in the
  file header and §6). It drives the *shipped* code paths — `build_enhance_prompt` /
  `parse_enhanced_blocks`, the polish batch loop + retention guard (`apply_cleanup`),
  `extraction.rs` prompt/parse/validate, and `llm_commands::build_ask_system` — over three
  committed fixture transcripts (`src-tauri/fixtures/bench/`): a 3-person standup, a 2-person
  budget review, and a 4-person product review (~30 segments each, seeded with disfluencies,
  figures, decisions, action items, and deliberately-unanswered questions).
- **Contract compliance** (hard pass/fail): Enhance = output parses as a JSON block array
  (the shipped fallback to untraced paragraphs counts as FAIL — it silently loses
  provenance); Polish = every batch parses AND zero retention-guard flags AND provenance
  preserved; Extraction = parseable AND ≥1 valid item AND ≤50 % of raw items dropped by
  validation.
- **Quality (LLM judge)**: 15 curated Ask questions (action items, decisions, "what did X
  say about Y", numbers recall) plus the Enhance notes are scored 1–5 by `claude-sonnet-5`
  against the sonnet-5 reference output, one-line justification required per score.
  **Circularity caveat**: Sonnet is judging closeness-to-Sonnet. This biases scores toward
  sonnet-ish phrasing and structure; treat judge scores as "how close to our current cloud
  quality bar", not as absolute quality. Contract compliance and factual-recall failures
  are the harder signals.
- **Latency**: wall-clock per call (`Instant`), full response, no streaming.
- **Peak memory**: peak RSS summed over all `ollama*` processes, sampled at 250 ms during
  each call (local models only).

## 3. Results

### 3.0 qwen3.5:4b is unusable through the app's Ollama path — measured, then excluded

Qwen3.5 **reasons unconditionally**. Through Ollama's OpenAI-compat endpoint — the only
endpoint the app's `OpenAiCompatProvider` speaks — the reasoning goes to a separate
`reasoning` field the provider doesn't read, and with the app's `max_tokens` budgets the
model never finishes reasoning, so **`content` comes back empty**:

- Full harness call (Enhance, standup fixture, the shipped prompt): **1360 s, 0 blocks,
  contract FAIL** — the entire 4096-token budget went to reasoning.
- Minimal probe (2-decision JSON ask, 400 tokens): `finish=length`, `reasoning` = 1543
  chars, `content` = empty. The Qwen3 `/no_think` soft switch is gone in 3.5 — prepending
  it changed nothing (still 1700 chars of reasoning). **A prompt-profile preamble cannot
  fix this.**
- Ollama's **native** `/api/chat` with `think: false`: perfect JSON in **41 s**
  (vs 183 s → empty with thinking). The model itself is fine; the app's plumbing can't
  reach that switch today.

The remaining qwen3.5 harness tasks were **skipped explicitly** (each call is ~20 min of
guaranteed contract failure; the failure mode is fully characterized above). Consequence:
qwen3.5 can only become the default after `fly-llm` grows Ollama-native `think` control
(or Ollama exposes it over the OpenAI-compat API) — see §5.

### 3.1 Score tables

_(from `target/llm-bench/*/results.json` + `judged.json`; default profile — i.e. the
prompts exactly as the app ships them)_

**Contract compliance** (hard pass/fail, 3 fixtures):

| Model | Enhance | Polish | Extraction |
|---|---|---|---|
| claude-sonnet-5 (reference) | 3/3 | 3/3 | 3/3 |
| llama3.1 (8B, baseline) | 2/3 | 2/3 | 3/3 |
| gemma3:4b | 3/3 | 2/3 | 2/3 |
| gemma4:e4b | 3/3 | 2/3 | 2/3 |

Every Polish failure was caught by the shipped lossless fallback (unparseable batch →
segments kept raw, zero retention-guard flags fired) — the no-loss design held for every
model.

**Judge scores** (claude-sonnet-5 scoring closeness to its own reference output, 1–5,
one-line justification each; see §2 for the circularity caveat):

| Model | Enhance (3 fixtures) | Ask (15 questions) |
|---|---|---|
| llama3.1 (8B, baseline) | 2.00 | 4.27 |
| gemma3:4b | 2.67 | 4.13 |
| gemma4:e4b | 2.67 | 4.33 |

**Latency (mean per call, this machine) + peak memory** (RSS = `ollama` + `llama-server`
processes, sampled at 15 s during the runs; VRAM from `/api/ps`):

| Model | Enhance | Polish (per fixture) | Extraction | Ask | Peak RSS | Peak VRAM |
|---|---|---|---|---|---|---|
| claude-sonnet-5 (reference) | 13 s | 10 s | 10 s | 4 s | n/a | n/a |
| llama3.1 (8B, baseline) | 261 s | 626 s | 312 s | 36 s | 8.7 GB | 2.2 GB |
| gemma3:4b | 357 s | 655 s | 487 s | 26 s | 7.3 GB | 2.3 GB |
| gemma4:e4b | 603 s | 891 s | 1160 s | 140 s | 5.9 GB | 2.4 GB |

Reading the table: local models on this hardware are **20–100× slower** than the cloud
reference on every task; gemma4:e4b's 140 s mean per Ask answer is not interactive.

### 3.2 The systematic small-model failure: spoken-number corruption

The budget fixture states the lease as "eighty four hundred" ($8,400/month) and SaaS
savings as "thirty two hundred" ($3,200/month) — the way people actually say numbers.
**Every local model corrupted both by 10×** (llama3.1: "$84k/month", "$32,200/month";
gemma3:4b: "$84,000", "$32,000"); claude-sonnet-5 got both right. For a meeting-notes
product whose Extraction feature exists to capture figures, this is the strongest
argument in the whole benchmark against a small local default. Judge lowlights (verbatim
one-liners):

- llama3.1 / ask q09 — 2/5: "Candidate reports $32,200/month, contradicting reference's
  $3,200/month figure, a likely fabricated/altered number despite correct owner and
  deadline."
- gemma3:4b / enhance budget — 2/5: "Multiple factual errors: lease is $8,400/month not
  $84,000, SaaS savings is $3,200/month not $32,000, action item owner for recruiter
  email is Alex not Dana…"
- gemma4:e4b / ask q10 — 2/5: "Candidate states July 20th while reference states July
  21st, a factual date discrepancy."

Ask quality is otherwise genuinely decent for all three (means 4.1–4.3/5): direct factual
recall is a small-model strength; structured multi-fact synthesis (Enhance) is where they
fall to 2.0–2.7/5 — coarse structure (gemma4 often returned ONE giant block), missing
metrics, misfiled action items.

## 4. Contract-failure examples (verbatim)

**llama3.1, Enhance (budget)** — emits a `"sources"` key *outside* its object, malformed
JSON, so the shipped parser falls back to untraced paragraphs (provenance lost):

```
  {"type": "ai", "markdown": "* Office lease renewed at $84k per month for 12 months"}, "sources": [13, 14, 16],
```

(Note the 10× number corruption inside the same line.)

**llama3.1, Polish (product)** — prose preamble is tolerated by the parser, but the JSON
itself has a delimiter error at char ~1789; batch dropped, all 29 segments kept raw:

```
Here is the cleaned transcript:

{"segments":[{"id":"seg001","speaker":"mic","start":0,"text":"Alright, this is the …
```

**gemma3:4b, Extraction (standup)** — `segment_ids` emitted as a *string* instead of an
array; serde rejects the batch, 0 items extracted:

```
    "speaker_key": "(mic)",
    "segment_ids": "[seg023]"
```

**gemma4:e4b, Polish (budget)** — returned a bare `[{"id","text"}…]` array instead of the
required `{"segments":[…]}` wrapper; batch dropped losslessly.

## 5. Recommendation: don't switch (and what would change that)

**Recommendation: keep `llama3.1` as the default local model for now.**

1. **No candidate wins decisively through the app's current plumbing.** gemma3:4b trades
   llama3.1's Enhance weakness (2/3 contract, judge 2.00 vs 2.67) for a new Extraction
   weakness (2/3 vs 3/3) at similar speed; gemma4:e4b matches gemma3's quality at 2–5×
   the latency (140 s per Ask answer). Nothing approaches the cloud reference.
2. **The most promising candidate is blocked on plumbing, not quality.** qwen3.5:4b
   produced perfect JSON in 41 s via Ollama-native `think: false` — by far the best
   small-model probe result — but is literally unusable (empty responses) through the
   OpenAI-compat path the app speaks (§3.0).
3. **All small models corrupt spoken numbers** (§3.2). Whatever the default, the honest
   product posture is that local = private-but-approximate and Anthropic remains the
   quality path.

**What would change the decision:** add Ollama-native `/api/chat` support (or `think`
control) to `fly-llm`, then re-run this benchmark with qwen3.5:4b included. Its probe
profile (fast, contract-clean, 256K context) suggests it could beat everything above.

### Prompt-profile adjustments (measured on the fly-core profile seam)

Variant runs (`LLM_BENCH_VARIANT=…`, single run per cell — treat small deltas as noise):

| Adjustment | gemma3:4b Extraction | llama3.1 Enhance |
|---|---|---|
| default | 2/3 (schema drift: `segment_ids` as string) | 2/3 (malformed JSON), judge 2.00 |
| **simplified contract** | **3/3**, item counts intact (12/9/8) | **3/3**, ~30 % faster (189 s vs 261 s) — but judge **1.67**: the contract's inline example ("Ship Friday", "my note") **leaks into the notes as invented content** |
| few-shot preamble | 3/3 but recall suppressed (5–6 items vs 11–13) | not run |
| "no preamble" instruction | 3/3, best item counts (12/12/9) | not run |

Takeaways: **simplified contract phrasing is the most effective single adjustment for
extraction-style tasks** (fixes schema drift without hurting recall); inline examples
are dangerous on llama3.1, which copies them into output — a future llama3.1 profile
should prefer an example-free simplified contract. The gemma3 standup extraction failure
was also fixed by the *no-preamble* variant, so that failure is likely stochastic —
another reason not to over-fit profiles to single runs. **The registry ships empty**: no
adjustment produced an unambiguous quality win for the current default model, and
per the registry's own rule, profiles are added only for measured, reproducible failures.

## 4. Contract-failure examples (verbatim)

TBD-FAILURES

## 5. Recommendation

TBD-RECOMMENDATION

## 6. Reproducing

```
# 1) reference outputs (needs the Anthropic key in env or the app keychain)
LLM_BENCH_MODELS="anthropic:claude-sonnet-5" \
  cargo test -p fly-app --test llm_bench -- --ignored --nocapture

# 2) candidates (pull models first: ollama pull qwen3.5:4b …)
LLM_BENCH_MODELS="ollama:llama3.1,ollama:qwen3.5:4b,ollama:gemma3:4b,ollama:gemma4:e4b" \
  cargo test -p fly-app --test llm_bench -- --ignored --nocapture

# 3) judge phase (sonnet-5 scores everything in target/llm-bench vs the reference)
LLM_BENCH_PHASE=judge \
  cargo test -p fly-app --test llm_bench -- --ignored --nocapture

# 4) prompt-profile experiments (rerun a candidate with a profile variant)
LLM_BENCH_MODELS="ollama:<best>" LLM_BENCH_VARIANT=simple LLM_BENCH_TASKS=enhance,extract \
  cargo test -p fly-app --test llm_bench -- --ignored --nocapture
```

Adding a per-model prompt profile after a decision: follow the checklist in
`crates/fly-core/src/prompt_profile.rs`.
