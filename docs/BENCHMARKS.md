# LLM Benchmarks — default local model decision

*Last run: 2026-07-13, on a Windows 11 laptop — RTX 3050 Laptop (4 GB VRAM), 32 GB RAM,
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

_(from `target/llm-bench/*/results.json` + `judged.json`)_

TBD-TABLES

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
