# Development notes and history

Historical implementation notes, including earlier behavior and experimental platforms.
Start with the [README](README.md) and [known limitations](docs/known-limitations.md).

Current source version: **0.8.3 — Model-independent prediction and multilingual input polish**.

## Current priority: Chinese and English input on Linux

Polish these two input languages before expanding language coverage or visual features.
English focuses on word endings, context-sensitive next words and short-sentence continuation.
Simplified Chinese focuses on continuous/separated Pinyin, unfinished readings, useful word/sentence
choices and continued typing after conversion. Both must retain literal input, deliberate adoption,
one-step completion undo, Space continuity and exact explicit submission, with or without a model.

Gradually extend basic Japanese Romaji/Kana and everyday fallback while keeping Chinese/English
as the core. Preserve the existing interface/translation languages; do not equate eight interface
languages with eight complete native input systems. Mixed Chinese/English,
broader vocabulary and real-model latency/quality still need work. Treat input-state correctness,
authored offline examples and real-model quality as separate acceptance gates.

The [local lexicon](data/lexicons/README.md) is the fallback when model capabilities are
unavailable, not a substitute for open-ended model generation. Publish local candidates immediately,
including while prediction is pending; disabled, missing, failed, timed-out or empty providers must
not block basic word/sentence adoption, editing or exact commits. Prioritize high-frequency gaps
over corpus size, preserve old ranks, and keep vocabulary data independent of model and host code.

The paired [English](tests/english_completion_quality.rs) and
[Chinese](tests/chinese_completion_quality.rs) regression suites provide a reproducible starting point,
not a general language-quality score. The first follow-up fixes separated Pinyin within existing
dictionary entries; see the [N47 audit](docs/bug-audit-bilingual-core-2026-09-24.md).

## Release requirement starting with 0.8.0

Version releases starting with **0.8.0** must include downloadable Linux packages in the
corresponding GitHub Release, not only source tags or expiring CI artifacts. Initially target
the validated Ubuntu 24.04 / amd64 baseline: attach the `.deb`, binary `.tar.gz` and matching
SHA-256 checksum files, with installation/upgrade instructions and known limitations.
Build from the exact, clean tagged source; require the same commit's CI, package-content
checks and clean-container installation checks to pass before publication. Other platforms
are added only after their own acceptance, not implied by the shared version number.
This is a release requirement, not a claim that automatic publication is implemented or
that existing 0.7.x tags already have assets. See [Linux packaging](docs/linux-packaging-data.md).

### 0.8.3 release preparation (2026-10-06)

The six accumulated development sections immediately below are included in
0.8.3; their "Unreleased" headings and validation statements retain their
historical scope. See [0.8.3 release notes](docs/releases/0.8.3.md) for the
consolidated changes and limits. Rust/lockfile, both macOS bundles and Android
share version 0.8.3, with platform build number 33. Only the root package
version changes in Cargo.lock; third-party dependency versions are unchanged.
Publication requires the exact clean tagged commit's full CI and package /
clean-container installation acceptance, followed by permanent Linux assets.
The deferred local IBus daemon abort is not declared fixed by this release.
Release review also aligns the legacy adapter's English language aliases with
the shared protocol policy: `en`, `en-US` and `en_GB` retain the same raw
indentation when an alternative completion differs from the local conversion.
The existing focused unit regression now checks all three IDs. Local release
logs are retained at `/tmp/suzaku-release-0.8.3.v3zXu7/`.
After the version and alias changes, all 1,226 Rust tests pass again with zero
failures and 27 opt-in skips. Strict all-target/all-feature Clippy, formatting,
source digest, 21 tensor checks, six borrowed-signal-lifetime checks, 23 private
diagnostic checks, shell/Python syntax and release metadata checks pass. The
full-history and staged secret scans pass. Local ShellCheck is unavailable;
the unchanged required CI step and full native/package gates remain pending.

### Unreleased — Prediction preservation and English boundary follow-up (2026-10-06)

- Apply provider-independent draft semantics in the worker, after structural
  protocol validation and before either display limit. New and legacy custom
  providers cannot drop an English raw prefix/indentation, echo the unchanged
  draft or expose generated request metadata. Filter individual invalid rows;
  an entirely rejected batch leaves the local pool intact with `NoCandidates`.
- Keep raw English text independent of the best local completion. Only positive
  dictionary-prefix evidence triggers the incomplete-word guard; mere absence
  from the fallback dictionary is not a universal vocabulary ceiling. CJK
  phonetic/homophone conversion remains allowed. Exact preserved Unicode and
  typed JSON prefixes are excluded from generated-metadata checks, even when
  an alternative conversion differs from the primary local conversion.
- Restore separate English word projections when a model sentence inserts extra
  horizontal spacing after an already typed separator. Preserve ASCII, NBSP and
  Unicode horizontal spacing in the payload, including a model separator after
  a known complete word. Do not cross vertical separators or split technical
  tokens. A protocol-to-word-to-explicit-commit test checks exact whitespace.
- Opening brackets/quotes now only continue an existing English word boundary;
  a wrapper inside a protected URL/path/identifier cannot expose hidden authored
  phrase context. Normal/nested wrappers and whitespace after protected tokens
  still work across short drafts, long local windows and own-session commits.
- Preserve original red logs in `/tmp/suzaku-prediction-followup.ol75Et/`:
  two spacing failures and five protocol/protected-context failures. The first
  shared guard over-restricted unlisted words, caught by three existing fixture
  failures in `focused.log`; refining the code (not weakening assertions) passes
  all 52 focused integration tests in `focused-refined.log`. Extra policy units
  cover CJK typed metadata and English alternative completions.
- Full all-feature Rust validation passes 1,226 tests with zero failures and
  27 existing opt-in/fixture tests ignored (`rust-all.log`). Strict all-target,
  all-feature Clippy passes (`clippy.log`), as do the 21 tensor-validator tests,
  source-digest, formatting and diff checks. Maturity scores remain unchanged.
- No vocabulary data, model/service installation, model sampling/network format,
  personal input source, native event routing, release version, commit or push
  changes. No real-model quality or full native/GTK/physical-keyboard acceptance
  is claimed; the previously deferred daemon investigation remains deferred.

### Unreleased — Model-independent prediction protocol v1 (2026-10-04)

- Introduce `prediction::PredictionProvider` with serializable versioned requests
  and responses, caller-owned request IDs, complete-draft replacement semantics,
  language/session input, typed word/sentence candidates and bounded output limits.
  No model name, endpoint, credential or UI annotation is part of the contract.
  Existing context/input/output budgets remain unchanged; callers may reduce the
  candidate count and generated-character budget, not expand global limits.
- Connect the engine worker, Linux host and standalone panel to the new contract.
  Validate request/response envelopes independently of providers while keeping
  local revision rejection, one worker, cancellation and immediate local fallback.
  Preserve the legacy completion provider and struct-literal APIs through an
  explicit adapter. Candidate ranking accepts neutral types with a legacy wrapper.
- Move chat prediction instructions, language/prefix rules, candidate content
  parsing and schema into `prediction/prompt.rs`. HTTP adapters retain discovery,
  authorization, transport, service envelopes and finish-state checks. Keep the
  small-model prompt and network format unchanged; translation remains separate.
- Add 15 protocol/engine integration tests, four controlled HTTP contract tests
  and one legacy English-spacing regression. The compatibility adapter preserves
  raw English indentation even when the local conversion is a different word;
  malformed legacy rows cannot erase other valid candidates. Dual HTTP envelopes
  and arbitrary model names produce equivalent task data and typed responses.
- Full all-feature Rust validation passes 1,209 tests, with zero failures and
  27 existing opt-in/fixture tests ignored. The 21 tensor-validator tests,
  strict all-target/all-feature Clippy, source-digest, formatting and diff checks
  pass; maturity scores are unchanged.
  Logs are retained at `/tmp/suzaku-prediction-protocol.TbEH8f/`. All model I/O
  validation uses owned synthetic providers/loopback fixtures, not real model
  services. No complete native gate, GTK or physical keyboard test is claimed.
- This creates a backend-independent extension point, not improved model accuracy,
  streaming generation, a new server, broader text collection or real-device
  acceptance. No vocabulary, model installation, personal input source, version,
  commit or push is changed by this work. See [the protocol contract](docs/model-providers.md).

### Unreleased — Chinese partial-reading spacing (2026-10-04)

- Fix disappearing Chinese word/sentence candidates when an unfinished Pinyin
  draft gains trailing horizontal whitespace: `zhongw `, `shu ru f  ` and
  adopted Han text such as `发音 ke y ` now retain their existing completions.
  Match through a trailing-spacing-trimmed view, without changing decoder byte
  offsets, literal drafts, internal syllable boundaries, quotes or line breaks.
  Keep adopted Han spacing and long frozen prefixes exact. Required separators
  still distinguish the primary `xian → 先` and `xi an → 西安` readings.
- Seven independent integration tests first produced five failures and two
  passing negative-boundary checks; the original log remains at
  `/tmp/suzaku-pinyin-spacing.A2r656/red.log`. After the fix all 44 focused
  tests passed, including provider failures, all eleven recommended packs and
  the previous Japanese reading-boundary regressions. Add one direct unit test
  for mandatory separators and exact consumed-byte offsets.
- The private bilingual fixture adds three seeds/six word-or-sentence routes
  for actual Space events, numeric adoption, immediate BackSpace undo, continued
  input and a single explicit Return commit. `SUZAKU_NATIVE_BILINGUAL_ONLY=1`
  selects this existing bilingual gate without executing unrelated lifecycle
  diagnostics; the default full gate still runs all cases as before.
- Both focused native modes pass all 102 bilingual workflows: default and
  `SUZAKU_IBUS_INLINE_PREEDIT=1`. The private desktop-portal helper's startup
  `Not connected to the ibus bus` warning remains visible in both logs; the
  owned input daemon/host complete the assertions and both commands exit zero.
  This is private IBus validation, not GTK/physical-keyboard or personal-install
  acceptance, and not a rerun of the previously failed full native gate.
- Full Rust validation passes 1,189 tests (27 ignored). Strict all-target,
  all-features Clippy, formatting, diff checks and the 21 tensor-validator
  tests also pass. Logs remain under `/tmp/suzaku-pinyin-spacing.A2r656/`;
  maturity scores are unchanged.
- Lexicon contents, candidate budgets, model protocols and native key routing
  are unchanged. The intermittent upstream IBus abort investigation is deferred
  at the user's request, not fixed or reclassified as a passing complete gate.
  No version, commit, push, personal installation or desktop input source changes.

### Unreleased — Japanese reading boundaries (2026-10-04)

- Fix disappearing partial-word/sentence choices after trailing horizontal
  separators (`nihong `, `JYUNB\t`, `jouk` plus NBSP). Probe incomplete Romaji
  vowels before those separators, using the same seven vertical boundaries as
  composition conversion. Literal drafts remain exact; do not join a syllable
  across internal whitespace or a line boundary.
- Fix conversion splitting an already recognized mixed-script word: adopting
  `同じ` then typing `kanji` incorrectly produced `同時間じ`. The primary
  conversion and tail completion now share a longest-prefix decision that
  preserves known written words/Katakana when longer than a phonetic match.
  Longer readings and equal-length original dictionary/homophone priorities
  still win. `同じ漢字` / `同じ感じ`, `後で` and `別の` have fixed regressions;
  no phrase-specific decoder branch or new model dependency is introduced.
- Five new independent tests first produced four failures and one passing
  negative-boundary test. After the fixes all 23 Japanese focused tests pass,
  preserving seven horizontal variants, seven vertical boundaries, unknown
  text, adoption, exact commits and undo. The provider-error and all-eleven-pack
  fixtures add fixed partial-space and mixed-script examples. Lexicon files,
  candidate budgets, native key routing and model deadlines are unchanged.
- Private IBus coverage adds six partial-Romaji Space word/sentence workflows,
  adopted Katakana plus partial-tail Space, and `同じ` adoption/continued
  Romaji/homophone adoption/undo/one explicit commit. Validation logs, including
  the original failures, are under `/tmp/suzaku-japanese-boundaries.mvgN0w/`.
  The focused Japanese fixture passes in both default and inline-preedit modes.
- Full Rust validation passes 1,181 tests (27 ignored), including typed provider
  failures and all eleven recommended packs. The 21 tensor-validator tests,
  strict all-target/all-features Clippy, formatting and diff checks pass;
  maturity scores are unchanged.
- The complete default IBus gate **failed**, not passed: after presentation
  checks, tray activation encountered a daemon SIGABRT at 8.738 seconds
  (`ibus-daemon` PID 3441209, return code -6, bus disconnected). The log shows
  the invalid-sender assertion followed by `call_in_idle_cb`'s missing-vtable
  assertion, matching the previously recorded abort signature below. This
  happened before the complete gate reached the Japanese fixture; no recent
  kernel OOM was logged. The specific sender-lifetime cause remains unproven,
  and the two focused Japanese passes do not turn this into a full native pass.
  Preserve `native-default.log`; do not retry until green, raise deadlines or
  alter installed dependencies. No version, commit, push, personal installation,
  input source or GTK fixture is changed.
  Installed IBus `1.5.34~rc2-1` / GLib `2.88.0-1ubuntu0.1` match the earlier
  investigation. Preserve the newly timestamped Apport report privately as
  `ibus-daemon.crash` alongside the logs (mode 0600); it is not uploaded or
  added to Git, and has not yet been unpacked or analyzed as a new core.
  Report SHA-256: `aa9a38cde12f54c5d0334d9f687e4945d94638fe337b28d671a5f83d19652f67`.

### Unreleased — Japanese choices fallback (2026-10-04)

- Extend daily Japanese comparisons, preferences, conditions and alternatives
  through independent authored data: 48 readings, 24 contexts and 48 sentences,
  for totals of 122 / 62 / 123. Preserve all 74 released readings and
  38 continuation contexts with new full historical fingerprints, in addition
  to the original 26-reading / 14-context snapshots. The preceding uncommitted
  English/Chinese choices work stays intact; no model-specific dispatch is added.
- Six new independent tests distinguish a verbatim Kana literal from a converted Word,
  specific Romaji prefixes from broad prefixes, and authored continuation from
  arbitrary sentence-level reading conversion. Exercise uppercase/Hiragana and
  horizontal separators, word/sentence choices, adopted words plus particles,
  exact commit/undo and negative unknown/line-boundary cases.
- Tests-first reproduced the missing choices: five new tests failed and the
  negative-boundary test already passed; the provider-fallback red run also
  failed on missing `KONOMI → 好み`. Both logs are preserved before appending
  data. Expectations are not generated by reading the production JSON.
- Provider-failure and all-eleven-pack tests add Japanese choices independently
  of the resource contents: four mixed-spelling word/sentence examples plus two
  adopted-particle progress examples in the pack gate. The focused run passed
  41 tests across new/old Japanese vocabulary, sentence progress, resources,
  provider failures and all packs. Missing/error/timeout/503/empty responses
  are synthetic typed errors, not live-model evidence.
  Private IBus exercises numeric word/sentence adoption,
  exact undo, literal Space, adopted-word particles and conflicting tails. No
  personal installation, input source, GTK fixture, model or timeout is changed.
- The first private IBus run exposed a genuine entry gap: `ひかく` works in
  the engine, but the native empty-draft path rejected its initial `ひ`.
  Accept only alphabetic Hiragana in Japanese mode; leave other languages,
  non-reading idle keys, application chords, hard-bypass purposes and unfocused
  events unchanged. Dedicated synchronous native checks assert these boundaries
  and no implicit commits. This is not a JIS Kana-layout implementation.
- After that fix, the native run found an old workflow displaced by new word
  competition: `準備gadeki` still offered its authored continuation, but only
  on page two. An independent engine red test reproduced the lost first-page
  choice. Rank non-primary Japanese base Sentence conversions below authored
  continuations, using the existing Chinese weighting rule; preserve the
  primary conversion, Word/Unspecified weights, literal, quotas and list limit.
  This also applies to non-primary sentence conversions from Japanese packs,
  not only mixed Katakana variants. No candidate text, kind or source changes.
  The first engine rerun then caught an incorrect new-test label: a pure
  phonetic Katakana variant is Unspecified, not Sentence. Correct the fixed
  expectations per variant, without changing classification; retain that log
  as `focused-final.log`. All seven progress tests then pass in
  `progress-fixed.log`, including the original first-page failure.
  Add the same fixed word/sentence expectation to provider-failure coverage
  and both adopted/Romaji variants to the all-eleven-pack progress gate.
- Both focused Japanese native modes and both complete private IBus modes
  (default and `SUZAKU_IBUS_INLINE_PREEDIT=1`) pass after these fixes. The
  Japanese fixture covers first-key/bypass boundaries, 16 daily word/sentence
  workflows, five positive/five conflicting-tail continuations, homophones,
  `nn`, adopted Katakana and later-page literal submission. These are private
  IBus events, not a physical-keyboard or personal-desktop acceptance.
- Validation logs and red/green results are kept under
  `/tmp/suzaku-japanese-choices.bo3VDg/`. The final all-features Rust run passes
  1,176 tests (27 ignored), including the provider/pack ranking additions;
  strict all-target/all-features Clippy, formatting and diff checks pass.
  The 21 tensor-validator tests pass without raising maturity scores.
  This remains bounded fallback, not full Japanese
  morphology, arbitrary Katakana-to-Kanji conversion or a longer Japanese draft
  window. No version, commit or push is changed.

### Unreleased — Daily choices bilingual fallback (2026-10-04)

- Append-only `daily_choices` covers comparisons, preferences, conditions and
  alternatives without model-specific or language-algorithm changes. English
  adds 24 contexts / 48 authored sentences and only three indexed forms
  (`depends/hurry/simpler`); Chinese adds 65 readings / 24 contexts / 48 sentences.
  Totals: EN 6,127 indexed forms / 521 contexts / 1,006 sentences; ZH 2,432
  readings / 515 contexts / 1,028 continuations. Japanese and all recommended
  pack contents are unchanged; the 6,144 English index budget stays fixed.
- Eleven new independent engine tests cover both branches, spelling variants,
  partial words/readings, first-page word/sentence choices, preserved literal
  text/long prefixes, editable adopted drafts, continuation and exact commit/undo.
  New snapshots freeze all v0.8.2 English ranks, contexts and sentences and all
  Chinese readings/contexts, in addition to every existing historical snapshot.
- Both tests-first runs failed all five new tests before their language's data
  was appended; logs remain in `/tmp/suzaku-vocab-choices.XZixA7/`. Provider-error
  and all-eleven-pack gates add five fixed bilingual variants. These synthetic
  missing/error/timeout/503/empty results are not live-model quality evidence.
  The first full run then found one stale Chinese unit-test total (2,367 versus
  the new 2,431); only that current-total assertion was updated. Every historical
  fingerprint and per-entry reachability/uniqueness assertion remains intact,
  and the failing full-run log is retained as `rust.log`.
  The next full run (`rust-final.log`, retained despite its original filename)
  exposed a real data gap: `这个词/这个意思` existed, but standalone `这个` did not.
  After adopting `我更喜欢`, `zhe ge` expanded to longer words and could not
  continue the authored sentence. Append the basic `zhe'ge → 这个` reading;
  keep the original sentence expectations and separately test it alone and
  after different adopted prefixes. No phrase-specific decoder logic is added.
  The focused rerun passed all eleven new engine tests, then caught a new
  fixture mislabeling the complete multiword `我更喜欢这个` as Word rather than
  Sentence. The bilingual word/sentence helper now explicitly checks the old
  longer Word `我更喜欢这个词` alongside the intended authored sentence; the
  dedicated demonstrative test retains exact full-conversion Sentence checks.
- The private IBus vocabulary fixture adds four seeds / eight word-or-sentence
  adoption, undo, continuation and single-commit routes (96 bilingual routes
  total). Its reset now requires a newer context plus an empty draft before
  subsequent language controls or typing, closing an asynchronous fixture race.
  Native typing tests internal double spaces; engine tests additionally accept
  leading spaces supplied as a draft, since Space on an empty native draft
  correctly passes through. No GTK scenarios or timeouts are changed.
- Final serial validation passed 1,168 non-ignored all-features Rust tests,
  strict all-target Clippy, formatting, diff checks and 21 tensor tests. The
  27 opt-in Rust tests remain separate. The successful complete Rust run is
  `rust-verified.log`; earlier failures are not reclassified as passes.
  All eleven recommended packs passed together, including the new bilingual
  variants and the adopted `这个` sentence route. Default and inline-preedit
  full private IBus gates passed (`native-default.log` / `native-inline.log`),
  each including all 96 bilingual vocabulary workflows. These two runs do not
  establish a fix for the previously investigated upstream IBus crash.
- Tensor source evidence and the current dirty-snapshot base (v0.8.2 commit
  `a019b7c`) are refreshed without changing maturity scores or historical runs.
  No GTK gate, physical-keyboard/personal-desktop test or live-model quality
  evaluation was run this round. No personal installation, input source,
  version, commit or push is changed.

### 0.8.2 release preparation (2026-10-04)

The accumulated development sections below are included in 0.8.2; their
"Unreleased" headings and validation statements retain their historical scope.
See [0.8.2 release notes](docs/releases/0.8.2.md) for the consolidated changes,
platform limits and installation instructions. Rust/lockfile, both macOS bundles
and Android now share version 0.8.2, with platform build number 32. Only the
root package version changes in Cargo.lock; dependency versions are unchanged.
CI also explicitly validates the functional tensor and its 21 fixture tests.
Publication still requires the exact commit's complete CI and clean-container
package acceptance, followed by permanent Release attachments. Local validation
logs for this preparation are under `/tmp/suzaku-release-0.8.2.H7rcvj/`.
After the metadata bump, 1,155 ordinary all-features Rust tests and strict
all-target Clippy passed again (27 opt-in tests stay separate), together with
formatting, shell syntax, 21 tensor tests and staged/full-history secret scans.
Local ShellCheck is not installed; the mandatory CI gate remains enabled.

The first [0.8.2 CI run](https://github.com/chiharu-kiryu/Suzaku-map/actions/runs/37182510232)
passed Rust, both complete IBus modes, UI, native layout/candidate gates, the
complete GTK vocabulary gate and physical keyboard checks. Stock-popup navigation
then failed: its old fixture expected row 9 on a second Down click with ten
candidates, although the new page-start contract correctly retained row 6.
The failed log remains at `ci-failed.log` in the preparation directory above.
The fixture now explicitly requires a partial second page and expects Down
`6,6,6` / Up `0,0,0`, while preserving all real XTest, accessibility, focus,
literal draft and exact-save assertions. It additionally checks every observed
navigation snapshot for page-start selection and unchanged draft/candidate state.
All 47 stock-popup workflows, including 18 ordinal-glyph checks, passed locally
after this fixture-only correction (`popup-fixed.log`, owned files at
`/tmp/suzaku-app-qa.eBXOyy`). Product code and timeouts were not changed; no failed
CI is reclassified as success, and final-commit CI/packaging must run again.

### Unreleased — Japanese sentence progress (2026-10-04)

- Fix authored sentences disappearing once Japanese input advances beyond an
  exact trigger: `予定ga`, `nihongo wo` and `JYUNBI GADEKI` now retain matching
  sentences. Match the whole converted draft, prefer longer applicable contexts
  stably, deduplicate and bound added sentence choices. Existing exact-trigger
  order remains covered independently; unknown prefixes/conflicting tails are
  never removed to recover an unrelated sentence.
- Continue consuming horizontal Romaji reading separators only in converted
  choices, while the literal draft remains exactly selectable. Preserve LF,
  CR, CRLF, vertical tab, form feed, NEL and Unicode line/paragraph separators;
  conversion and sentence matching must not silently join separate lines.
- The first full regression run exposed another real boundary in a new rail-pack
  case: `shuudennha` became `終電んは`, although `shuudenn` already ended in one
  `ん`. Keep a completed `nn` as one `ん` before the next consonant; preserve
  `nna`, `nni`, `nnya`, `nnna`, repeated n and both explicit apostrophe rules.
  Fixed unit expectations cover the intermediate `shuudennh` state too, with
  a private IBus `denn` → `dennwa` → `電話` adoption/undo/commit workflow.
- No new vocabulary or model-specific path: the implementation consumes the
  active lexicon. Six new independent engine tests cover 21 particle/progress
  spellings, every literal prefix of three fixed sentences, adoption, negative
  boundaries and exact commits/undo. Ten rail-pack progress spellings exercise
  the same behavior with all eleven recommended collections enabled.
- Red runs reproduced both issues: two unit failures and five of six new engine
  test failures. The subsequent full-run rail failure and focused `nn` red test
  are also retained; the original test was not removed or relaxed. Logs remain
  in `/tmp/suzaku-ja-progress.LS2hah/`. The first
  purported inline invocation used an unrecognized environment variable and
  was another default-mode run (`native-extra-default.log`); the corrected
  `SUZAKU_IBUS_INLINE_PREEDIT=1` run is recorded separately as `native-inline.log`.
- Final validation passed 1,155 non-ignored all-features Rust tests, strict
  all-target Clippy, formatting, diff checks and 21 tensor checks. The 27 opt-in
  Rust tests remain separate. This includes the ten rail progress cases in the
  all-eleven-pack fixture; the first failing full run is not reported as a pass.
- Focused private IBus passed again after the `nn` fix in default and inline modes, retaining the old
  Japanese cases and adding three sentence-progress and three conflicting-tail
  workflows plus the `nn` consonant transition, with numeric adoption, exact
  spelling undo and single Enter commits.
  Four focused GTK/XTest workflows passed, including new adopted-word + Space
  + `ga` sentence selection/undo and observed exact editor saves for `予定` and
  `準備`. Final owned editor artifacts: `/tmp/suzaku-app-qa.G0s3jy` (the earlier
  pre-`nn` run remains at `/tmp/suzaku-app-qa.klFSAZ`).
- This round does not rerun full IBus, full 89-workflow GTK, personal installation,
  physical keyboard, native Wayland or live-model quality acceptance. It does
  not infer readings of unknown Kanji in sentences (`予定がkima`), perform full
  morphological analysis, scan arbitrary history or extend the Japanese
  256-scalar local window. Version, dependencies, personal input source and
  service installation remain unchanged; no commit or push is performed.

### Unreleased — Gradual Japanese everyday input (2026-10-04)

- Japanese now begins a bounded expansion alongside the English/Chinese core.
  Append 48 authored readings and 24 pairs of continuations: 74 readings,
  38 contexts and 75 continuation records in total. The original 26-reading
  fingerprint remains, and a new fingerprint freezes all 14 original contexts.
  All English/Chinese resources and recommended packs remain unchanged this round.
- Fix terminal `nn` before punctuation/non-Romaji text duplicating `ん`, recognize
  curly `n’` alongside `n'`, and accept `jya/jyu/jyo` aliases. ASCII-only case
  normalization protects unrelated Unicode such as `Ω`, `Σ` and `İ`; uppercase
  Romaji still works. Preserve the old `nn'` interpretation rather than leaking
  an apostrophe through the new terminal-n folding branch.
- Exact final readings after known prefixes now retain homophones:
  `watashihakanji` offers `私は漢字` and `私は感じ`. Known adopted spellings,
  including `ニホンゴ`, can precede partial Romaji (`wobenky` → `を勉強`)
  without replacing that spelling. Prefix matching is allocation-free and bounded;
  arbitrary unknown prefixes, digits and emoji are not skipped to find a suffix.
- Independent daily tests cover complete/partial Romaji and Hiragana, uppercase,
  first-page words/sentences, original text, exact single commits and undo.
  Typed missing/error/timeout/503/empty providers cover the new vocabulary without
  pretending to be live-model evidence. Full-pack tests check new Japanese words
  alongside the existing rail collection. New private IBus and GTK vocabulary
  scenarios cover actual key delivery and editable adoption; their execution
  results are recorded below, separately from test-source coverage.
- Initial red tests reproduced lost suffix homophones, failed adopted Katakana
  completion and repeated `ん`. A data test additionally assumed literal input
  must be on page one; `as` legitimately retains that choice on the next page.
  The test now searches the full list only for Literal, while words and sentences
  still require page one for complete/specific spellings. The next run exposed
  the same overbroad assumption for a single Kana (`あ`); those broad prefixes
  instead require reachability within the bounded list, retaining old word ranks.
  A native keyboard case explicitly selects the later literal row. Logs and
  retained failures: `/tmp/suzaku-japanese.3vStYH/`.
- Final validation passed 1,147 non-ignored all-features Rust tests, strict
  all-target Clippy, formatting and 21 tensor checks; 27 opt-in Rust tests remain
  separate. Default-mode full private IBus passed, including 88 EN/ZH workflows
  and the new Japanese suite (eight daily word/sentence routes, two homophones,
  adopted Katakana and later-page literal). The focused Japanese suite also
  passed with inline preedit; this is not an inline full-suite claim.
  Four focused Japanese GTK/XTest workflows passed with actual key delivery,
  observed editor buffers and exact saved commits, including uppercase `JYUNBI`.
  All new cases are also included in the default full vocabulary gate, now
  86 vocabulary cases plus three Chinese sentence-progress controls; that
  complete 89-workflow GTK gate was not rerun this round. No personal-desktop,
  native Wayland, physical-keyboard or live-model quality acceptance is claimed.
- This is not a complete morphological analyzer or arbitrary Kana/Kanji converter.
  The 256-scalar local/model budget, Japanese long-draft limitation, model interfaces,
  candidate capacities and Space-as-reading-separator contract remain unchanged.
  No personal installation, input source, dependency, version, commit or push is changed.

### Unreleased — Everyday objects bilingual fallback (2026-10-04)

- Append-only `daily_objects` data covers finding/placing belongings, borrowing
  and returning items, small requests, quantities and sizes. English adds 24
  contexts and 48 authored sentences, reusing existing vocabulary except seven
  explicit forms: `chairs/cups/mine/plates/scissors/tape/yours`. Chinese adds 64
  readings, 24 contexts and 48 continuations. Current totals: 6,124 English
  indexed forms / 497 contexts / 958 sentences; 2,367 Chinese readings / 491
  contexts / 980 continuations. Japanese and recommended pack contents are unchanged.
- Vocabulary remains independent JSON and a model-unavailable fallback, not a
  replacement for open-ended generation. No decoder/model branches, candidate
  capacities or index budgets change. New snapshots freeze the previous 15
  English layers' ranks, all 473 contexts/910 sentences and the previous 2,303
  Chinese readings/467 contexts; every older snapshot remains in place.
- Independent fixed expectations cover all new expressions, both sentence
  branches, partial/separated Pinyin, case/apostrophes/spacing, long mixed
  prefixes, literal choices, exact commits and undo. Six new bilingual scenarios
  join typed provider-error fallback, native IBus and GTK/XTest vocabulary gates.
  Full-pack regression also checks new built-ins and the old cooking pack together.
- The first package regression caught a real new-data collision:
  `please pass me the` already belongs to the cooking pack. Only the new built-in
  context was changed to `could you pass me the`; the shipped pack and its
  `colander/spatula` choices remain intact. Logs, including the original failed
  run, are retained in `/tmp/suzaku-vocab-objects.7nYSBX/`.
- The first GTK full-vocabulary run stopped before delivering `There's a spare ch`:
  its typing fixture explicitly rejected uppercase ASCII. The helper now sends
  uppercase letters through the existing physical Shift/key press/release path;
  lowercase, digits and supported punctuation retain their old mapping, and
  unsupported characters are still rejected. The uppercase scenario and exact
  text assertions remain unchanged. A display-free mapping check verifies this
  fixture correction; the original failure and owned QA artifacts are retained.
- Final source validation: 1,138 non-ignored all-features Rust tests, strict
  all-target Clippy, formatting and 21 functional-tensor tests passed. The 27
  opt-in tests remain distinct from that Rust count. Default and inline-preedit
  full private IBus gates passed, each including 88 bilingual vocabulary
  word/sentence adoption/undo/continuation/commit workflows. The default gate
  still emitted the previously investigated IBus invalid-sender GIO warning but
  completed successfully; this data change does not claim to fix that dependency
  issue. Source fingerprints and evidence are updated without raising scores.
- After the fixture correction, the full isolated GTK editor vocabulary gate
  passed both mixed-language partitions: 41 vocabulary cases plus three Chinese
  sentence-progress controls in part 1, and 41 vocabulary cases in part 2 (85
  workflows total). This includes the uppercase English example and all six
  added bilingual cases, with real XTest delivery, observed editor buffers and
  exact saved word/sentence commits. The original per-session deadlines and all
  assertions remain unchanged. Fourteen editor-observer unit checks also passed.
  Xvfb/XTest is not a physical-keyboard, native Wayland or personal-desktop test;
  no live-model quality, package installation or GPU visual acceptance is claimed.
- This source expansion does not change the personal installation, version or
  desktop input source. Rebuilding/reinstalling is needed to use the compiled-in
  additions on the desktop; no release, commit or push is part of this round.

### Unreleased — Prose boundaries and consistent candidate paging (2026-10-04)

- English mode now completes words and authored sentences after explicit prose
  boundaries `—，。！？；：、`, without requiring an extra ASCII space. Examples:
  `前文，hel` → `前文，hello` / `前文，hello, how are you?`, and
  `说明—please sen` → `说明—please send` / `说明—please send me the details.`.
  Exact prefixes, case and spacing remain intact. Word, sentence, committed-context
  and long-draft paths share a linear boundary scan; ASCII URL/path/identifier
  syntax prevents a punctuation cut from exposing a fabricated natural-language
  suffix. The 160-scalar context and 256-scalar local/model budgets are unchanged.
  Final review caught repeated scanning of already-finished session history;
  the scan now starts at the current window's containing whitespace-delimited
  token. Structural tests exclude old history from the scanned slice, and 350
  boundary combinations compare it with the initial full-scan semantics. An
  unbroken token still retains its complete syntax for conservative URL protection.
- English adoption followed by an em dash retains pending preference feedback
  until explicit commit; undo still reverses it. ASCII hyphens, en dashes, URLs
  and identifiers do not become word boundaries for preference learning. This
  changes neither vocabulary resources nor model/provider coupling. Generated
  model suffixes with CJK punctuation remain whole sentence candidates, not
  automatically projected word candidates.
- IBus PageUp/Down, home-row Alt+H/L and IBus page events now select the adjacent
  page's first row, matching native panel arrows. Unavailable pages are inert:
  selection, revision, adoption undo and in-flight predictions remain unchanged.
  Draft page keys remain consumed; empty/private/unfocused targets retain the
  existing pass-through behavior. Row navigation and explicit adoption are unchanged.
- Red tests established missing mixed-prose completions, lost `hello—world`
  preference feedback, and first-page PageUp changing native revision. The new
  native fixture initially assumed a settings file already existed; it now saves
  the loaded default settings when starting fresh. A new test also incorrectly
  demanded typed sentence candidates from the short legacy nonmixed API; it now
  respects that existing untyped-word contract, while IBus and both long-draft
  modes still require first-page words and sentences. Original failures and
  subsequent validation logs are retained in `/tmp/suzaku-input-ux.6wkCJf/`.
- Final source validation: 1,126 non-ignored all-features Rust tests passed,
  with 27 native/opt-in tests kept separate; strict all-target Clippy, formatting,
  23 native-fixture diagnostic tests and 21 functional-tensor tests passed.
  Default and inline-preedit complete private IBus gates both passed on the final
  source, including the new EN/ZH three-entry page checks and four mixed-prose
  word/sentence adoption/undo/Space/exact-commit workflows. This is synthetic
  native protocol coverage, not physical keyboard, GPU redraw or real-model
  quality acceptance; those independent UI/installation gates were not rerun.
  The tensor source fingerprint is updated without increasing maturity scores.
- The earlier dependency crash investigation is parked, not declared fixed.
  No personal input source, installation, system dependency or version is changed.

### Investigation — IBus daemon abort recovered from the original crash (2026-10-04)

- Recovered the original Apport report at `/var/crash/_usr_bin_ibus-daemon.1000.crash`.
  Its 2026-10-03 21:57:46 +08:00 timestamp, daemon PID 2497370, parent fixture
  PID 2497361 and private `suzaku-sync-qa.NUQA9k/ibus.sock` address match the failed
  inline gate. Report SHA-256:
  `4f6da2c2944ba735473c638d2a9344f845cbaaa814e3aa5972ca58a3cd7bbd4b`.
  This establishes a daemon SIGABRT, not merely a client disconnect, OOM kill or
  outer test timeout. It does not imply that the user's desktop daemon crashed.
- The exact GIO Build-ID `a9791477ead64658f8936bdcc9e63a5d3f02a4f0` debug symbols
  confirm `call_in_idle_cb(user_data=0x0)`, `invocation=0x0`, at
  `gio/gdbusconnection.c:5474`. The assertion is
  `vtable != NULL && vtable->method_call != NULL`. Core-resident diagnostic text
  also retains the earlier `_g_dbus_method_invocation_new` sender-name validation
  failure. Thus an invalid non-null sender caused construction to return NULL;
  scheduling that NULL invocation led to the later assertion and daemon abort.
  The worker had already returned to its event loop: the actual sender bytes,
  method and corruption origin cannot be reliably recovered from this core.
- Scope: Ubuntu 26.04, IBus `1.5.34~rc2-1`, GLib `2.88.0-1ubuntu0.1`.
  This is not the Ubuntu 24.04 CI baseline. Official GLib
  [dispatch source](https://github.com/GNOME/glib/blob/2.88.0/gio/gdbusconnection.c)
  and [invocation construction](https://github.com/GNOME/glib/blob/2.88.0/gio/gdbusmethodinvocation.c)
  explain the observed failure chain; matching symbols were downloaded only into
  a private temporary directory, not installed as system packages.
- Still a hypothesis: the IBus worker's
  [sender assignment](https://github.com/ibus/ibus/blob/1.5.34-rc2/bus/dbusimpl.c#L1554)
  borrows the connection name while
  [connection destruction](https://github.com/ibus/ibus/blob/1.5.34-rc2/bus/connection.c#L80)
  can release it after removing the filter. GLib documents that filter removal
  does not wait for an already-running callback. However, the sender setter also
  validates before copying; proving this particular race requires observing the
  narrow lifetime window, not merely finding the static code pattern. No evidence
  establishes that the separately published engine-request cancellation fix is
  the same bug, or that a system upgrade alone resolves this failure.
- Diagnostic results: a no-Suzaku private IBus control survived 600 connection
  open/close cycles and 38,400 valid no-reply ListNames requests. The unchanged
  complete inline gate passed again, including N45/N44. These are negative
  reproduction results, not proof of a fix. Two temporary attempts to repeat N45
  on one bus stopped at iteration 2 with missing panel content-type events while
  the daemon remained alive; adding an inter-case drain did not resolve that
  harness-reuse limitation. Neither reproduced the historical daemon abort.
- Logs, temporary diagnostic drivers, matching symbols and the privately unpacked
  crash remain under `/tmp/suzaku-disconnect-investigation.TDhKk6/`. Raw core and
  environment data are not added to the repository or uploaded. The initial
  diagnostic-run approval timed out; the permitted retry executed normally.
  Next useful experiment is instrumented/ASan validation of sender ownership on
  a disposable upstream daemon, not changing product sleeps or suppressing errors.
  No production code, dependencies, personal input source, installation or version
  was changed. Ordinary Rust/Clippy and default-mode full gates were not rerun.

### Unreleased — Native recovery fixture diagnostics and cleanup

- Investigated the previous inline N45 connection loss. The original run discarded
  `ibus-daemon` stderr, and a failed panel-name release replaced the first engine
  query assertion. The surviving evidence cannot distinguish daemon termination
  from a single client disconnect; no production root cause is established.
- Keep daemon stderr and unbuffered Python output in the gate log. Before teardown
  changes process status, record the phase, local connection flags, daemon PID/exit
  status and the last eight owned children. No diagnostic D-Bus RPC is sent and
  full child arguments are not logged. Broken observation/stderr cannot hide the
  original error; this is a test-fixture diagnostic, not new product telemetry.
- N45 now owns and retires its delayed GLib source IDs, disconnects the panel
  handler and cancels remaining callbacks before the next recovery workflow.
  This replaces the old fixed 120 ms cleanup pump, not the 350 ms restoration
  stability check. Every cleanup stage is attempted, preserving the first failure
  and attaching later errors. PASS is printed only after cleanup succeeds.
- PyGObject can swallow asynchronous callback exceptions. Save the first callback
  exception and explicitly check it in the probe wait, stability loop and cleanup;
  bound additional error notes. Recheck every switch ACK after the stability loop
  so a late false result cannot pass merely because the engine was already right.
- Validation: 23 isolation/cleanup/diagnostic Python tests and six real GLib/C
  signal-lifetime tests passed, including callback failure and cancellation.
  Both complete default/inline private IBus suites passed, including N45/N44;
  neither reproduced the old disconnect. Shell syntax, ShellCheck, Python syntax,
  diff checks, 21 inventory tests and the refreshed source digest passed. Logs are retained in
  `/tmp/suzaku-recovery-audit.DwfJcp/`; the previous connection-loss log remains in
  `/tmp/suzaku-long-chain.WpURFn/ibus-inline.log`.
  The early scoped inline recovery run also passed. The initial bare `shellcheck`
  invocation was unavailable on PATH; validation used the already-installed
  `target/linux-lint.hijeqZ/root/usr/bin/shellcheck`, with no package download.
  No production Rust/C, personal installation/input source, model, vocabulary or
  version changed. This round does not rerun the ordinary Rust/Clippy suite or
  claim physical-keyboard, GNOME/Wayland, remote CI or release acceptance.

### Unreleased — Exact local-window boundary

- Follow-up on F64/F17/F18: the 256-scalar local-tail search did not inspect the
  separator immediately before a full-sized tail. Chinese could lose all local
  conversion for an otherwise decodable 256-character reading; English could
  discard the first complete word and lose its authored sentence continuation.
- Inspect one extra boundary scalar, then leave that scalar in the frozen prefix.
  The actual decoder window remains at most 256 Unicode scalars. Do not change
  the last-Han/CJK-prefix freezing policy, split protected tokens, normalize the
  preserved prefix or enlarge the whole-draft model/transport limits.
- Recorded red tests independently reproduced Chinese missing conversion and
  English missing sentence context at exactly 256; their short and over-budget
  controls passed. Added tests cover 254/255/256/257 boundaries, both candidate
  policies, Unicode/whitespace prefixes, exact commits and undo, protected tokens,
  and a valid 8,192-byte draft whose expanded conversion must be filtered intact.
  Controlled-provider tests retain the long-draft no-request check and a positive
  exact-256 short-draft request after shortening; no real model is called.
- The private IBus fixture adds 14 compact boundary workflows: owned full-text
  replacement or a final real host key event, word/sentence adoption, exact
  Backspace restoration and one full-payload Enter commit. A 257-scalar reading
  remains literal; English may complete its final word but not reuse frozen
  phrase context, including on later candidate pages. This is not physical-keyboard
  or personal-desktop acceptance, and does not require a personal installation.
- The 38 targeted long-draft/English-window/prediction tests passed after allowing
  the prediction suite's private local endpoint. The first sandboxed attempt hit
  `PermissionDenied` in an existing endpoint fixture; no assertions were relaxed.
  The subsequent complete all-feature Rust run passed 1,116 ordinary tests;
  27 opt-in gates remain separately enabled, not implicitly passed.
  Default private IBus passed the complete suite, including all 14 new boundary
  workflows. The first inline run passed those workflows and later input/host
  restart checks, then its existing IBus connection closed during the final N45
  password probe. The engine query failed, followed by the panel-name release;
  this was not the outer 120-second timeout. The unchanged isolated N45/N44
  recovery gate and an unchanged complete inline rerun both passed. Both final
  preedit modes include all 14 new boundary workflows. No cause of that connection
  loss is established; the first failure remains in `ibus-inline.log`, not erased
  by the passing `ibus-inline-recovery.log` and `ibus-inline-rerun.log`.
  Strict all-feature/all-target Clippy, formatting, diff checks, the 21 inventory
  tests and the refreshed source-digest check passed. No hardware/GUI acceptance,
  reinstall, packaging, version change, commit or push was performed this round.
  Logs, including both independent red tests, are in `/tmp/suzaku-long-chain.WpURFn/`.
  No vocabulary data, model provider, version, personal input source or install changed.

### Unreleased — Pointer deltas and English preference boundaries

- Follow-up from the sparse inventory, along F60/F36/F40 and F66/F16: signed-zero
  pixel-wheel and pinch events previously became ±1 through `signum`, causing
  horizontal/no-motion gestures to page candidates or resize the panel. Nonfinite
  deltas could also trigger actions or contaminate the settings scroll position.
  Check the original vertical value before taking its sign/narrowing precision;
  keep finite line-wheel magnitudes and existing nonzero gesture-phase behavior.
- The production event dispatcher now has 70 inert-event and 28 valid-event cases
  in the existing private native-action gate, covering candidate pages, Ctrl-wheel
  zoom, pinch and settings. These dispatch synthetic winit events inside owned
  Xvfb windows; they are not physical touchpad/touchscreen acceptance.
- English `hel -> hello` adoption no longer teaches that word when subsequent
  typing makes `hello.com`, `hello:world` or another attached period/colon token.
  Sentence endings, whitespace, closing punctuation and CJK separators retain
  their existing learning behavior. Canceled pending choices do not resurrect
  after deleting back to the old spelling; confirmed feedback still has undo.
  This is a choice-lifecycle guard, not a dictionary/parser dependency or new
  persistent cache. The private IBus fixture also checks ten token/punctuation
  continuations with exact commits and no learning while typing.
- Before the fixes, two ordinary gesture tests and the private native-action
  dispatcher gate failed; the new English preference regression recorded one
  erroneous learned entry for `hello.com` while its punctuation controls passed.
  Reproduction and validation logs are under `/tmp/suzaku-chain-audit.Vwd7sH/`.
  No personal installation, input source, vocabulary, model or version is changed.
- Validation: 1,110 ordinary all-feature Rust tests passed; 27 opt-in gates remain
  separately enabled, not implicitly passed. The updated private native-action
  gate passed all 70 inert and 28 finite-event checks, including real separate
  settings-window dispatch. Both complete private IBus preedit modes passed the
  ten English continuation cases and existing input/lifecycle/model-fixture checks.
  Real GTK/IBus/XTest candidate workflows passed 26 cases per default/synchronous
  client transport (52 total), including paging, word adoption/undo and exact long
  English/Chinese candidate clicks. Strict all-feature/all-target Clippy, formatting,
  diff checks, all 21 inventory tests and the refreshed source digest passed.
  Application fixture artifacts remain in `/tmp/suzaku-app-qa.KFvSY9/` and
  `/tmp/suzaku-app-qa.efxBx0/`; these owned X11 fixtures are not personal desktop tests.
- Retain intermediate diagnostics: the first ordinary run could not bind its
  fixture endpoints under the sandbox; the permitted local-fixture rerun passed.
  The settings positive control initially failed because the probe rendered the
  main window, which deliberately excludes settings. The fixture now opens a real
  settings window with a scrollable viewport and retains its nonzero-range and
  bidirectional assertions. A missing test-only enum import briefly blocked the
  first inline-mode attempt before it ran; the corrected compile and gate passed.
  Private Xvfb software-renderer DRI3 and IBus portal startup warnings are retained
  in logs; these are not proof of physical GPU/desktop acceptance.

### Unreleased — Architecture/function/implementation maturity inventory

- Preserve F01–F59 and index nine existing cross-cutting capabilities as F60–F68:
  native paging, caret placement, bottom layout, automatic platform layout, long
  drafts, payload budgets, preference frequency, resource decoupling and shortcuts.
- `docs/functional-tensor.json` records 10 architecture domains, 68 functions,
  47 scoped implementations and 77 sparse relationships (308 maturity coordinates).
  Implementation, regression, real-environment acceptance and delivery readiness
  remain separate ordinal scores; unknown is null, never an implied zero/pass.
- The read-only `scripts/functional-tensor.py` validates references, evidence,
  paths, score constraints and a digest of the declared implementation/test files;
  it can slice by architecture/function/implementation or export COO records.
  The network document explains the rubric and retains historical evidence.
- This is a documentation/data inventory, not a product behavior change. Historical
  source-build/Xvfb results, personal installations and releases are not promoted
  to current dirty-snapshot acceptance. No new Cargo/GUI run, installation,
  remote CI inspection, version change, commit or publication is implied.
- Inventory validation passed 21 isolated Python tests, all 68 unique function IDs,
  both identical Mermaid projections, direct documentation links and source-digest
  checks. Interactive slices and evidence disclosure were checked at 736/320px.
  Linux package documentation now includes the JSON inventory; shell syntax and
  static link checks passed, but packages were not rebuilt or installed this round.

### Unreleased — Input and offline-pack boundary fixes

- Pinyin tone digits now terminate before paired ASCII/CJK aside punctuation and
  Chinese em dashes. Numeric literals, decimals, versions, times and ranges retain
  their digits; conversion, original-text selection, exact commit and undo are covered.
- English contexts and phrase endings use startup-cached canonical keys for case,
  spacing and straight/curly apostrophes. Equivalent appended contexts keep the first
  authored entry. Sentence-only contractions are indexed and completed as whole words,
  preserving the typed prefix and existing vocabulary ranks.
- Offline registry records retain raw JSON until strict typed decoding, rejecting
  duplicate fields instead of silently using the last value. Startup isolates only
  damaged records; listing/mutations report corruption without overwriting the file.
  This enables the existing `serde_json/raw_value` feature, without a version change.
- Short-draft pack candidates now share the native byte/frame budgets already used
  for long drafts. Oversized alternatives are filtered from the actual candidate list,
  never truncated only in the mirror; literal input and rendered commit indices stay
  aligned. Model merges apply the final byte budget after preference ranking.
- Native numbered candidate cards commit the full text with one click even when
  their label is truncated. The independent editor keeps its existing first-click
  scrolling preview; stale-revision and duplicate-release guards remain in place.
- Independent `input_edge_audit_{chinese,english,packs,budget}` regressions cover these
  boundaries, including legal Unicode/JSON-escaped long entries and single-row limits.
  The initial failing reproductions are retained separately from validation results.
  No dictionary data, protocol limits, personal installation or version is changed.
- Local validation on 2026-10-03 passed 1,104 ordinary all-features tests (27 opt-in
  tests remain separate). After the final native-click change, all 228 ordinary panel
  tests, the expanded private native-action test and strict Clippy passed again.
  Both IBus transports of follow-caret/bottom/auto passed 26/33/36 workflows each:
  190 real private GTK/IBus/XTest workflows, including all three long-pack cases.
- First runs are retained in `/tmp/suzaku-input-audit.jMX1kg/`: regression testing
  caught an empty-list fallback and a test's incorrect assumption about the IBus
  12-candidate convenience switch. GUI failure persisted after strengthening the
  undo geometry-readiness check, exposing the native first-click-preview bug; its
  independent mouse/touch red test then passed after the guarded production fix.
  No failed action is retried, no deadline is extended, and these source-build gates
  do not establish personal GNOME/Wayland or physical-touch acceptance.

### Unreleased — Everyday social fallback

- Adds a separate `daily_social` English data layer and append-only Chinese readings for
  social check-ins, listening/support, invitations and polite declines. Both languages add
  24 contexts and 48 authored sentences; Chinese adds 64 readings.
- English adds only four indexed forms (`tiring`, `quiet`, `cafe`, `shall`), reaching 6,117
  indexed forms, 473 contexts and 910 sentences. Chinese reaches 2,303 readings,
  467 contexts and 932 sentences. Earlier ranks, homophone order, kinds and separator flags
  remain unchanged; additional fingerprints lock all pre-existing contexts and wording.
- Fixed-expectation tests cover offline first-page word/sentence choices, partial spellings,
  exact commits/undo, literal spacing/case/apostrophes and continued long drafts. Six added
  GTK scenarios exercise physical spelling/adoption/Space and saved commits; the full gate
  contains 79 workflows split into two mixed-language sessions with unchanged deadlines.
  Synthetic unavailable/timeout/HTTP-error/empty-model outcomes cover the new phrases too.
  These are finite authored expressions, not live message access or a model-quality claim;
  personal installation, Japanese resources and optional packs are unchanged.
- Local validation on 2026-10-03 passed 1,091 ordinary all-features tests (27 opt-in tests
  remain separate), strict Clippy, formatting checks and the complete 79-workflow private
  GTK gate (41 + 38). New phrases were actually selected, undone, continued with Space
  and saved exactly. The first language-only run caught an old aggregate count assertion;
  only its measured totals were updated, retaining every historical rank/content fingerprint.
  Private portal inhibition warnings remain visible; this is source-build validation, not
  a personal installation, native Wayland acceptance or real model-quality evaluation.

### Unreleased — Everyday communication and coordination

- Appends a model-independent `daily_coordination` English layer and Chinese daily phrases
  for clarification, confirmation, polite requests, sharing messages and coordinating help.
  Both languages add 24 contexts and 48 authored sentences; Chinese adds 64 readings.
- English reuses the existing vocabulary except for `hand` and `middle`, reaching 6,113
  indexed forms, 449 contexts and 862 sentences. Chinese reaches 2,239 readings,
  443 contexts and 884 sentences. The released 13 English layers and 2,175 Chinese readings
  are protected by additional rank/order fingerprints; earlier snapshots remain unchanged.
- Extends fixed-expectation word/sentence, literal-prefix, long-draft and unavailable-model
  regression coverage, plus six real GTK input scenarios and a physical consecutive-word
  continuation through `帮我看看` / `有没有`. At that stage the complete vocabulary gate
  contained 73 workflows in two mixed-language sessions with unchanged per-session deadlines.
  Native HTTP 503/deadline checks also cover the new English/Chinese phrases. An explicit
  `SUZAKU_NATIVE_QA_BIN_DIR` override selects installed host/probe/tool binaries; source-built
  panel tests remain distinct from the installed panel's application checks.
  These are finite fallback expressions, not live message access or model-generated suggestions.

### 0.8.1 — Native candidates and everyday bilingual fallback

- Adds optional English/Chinese study-writing, cooking-preparation, travel-lodging and team-work
  packs, bringing the bundled catalog to eleven collections. The eight new packages are version
  1.0.0 and contain 240 explicit English forms, 144 Chinese readings and 256 authored sentences,
  before built-in deduplication.
- Optional packs leave existing embedded vocabulary, prior ranks, Japanese data, decoder/model
  contracts and budgets unchanged. The new JSON can also be installed by the released 0.8.0 CLI;
  its existing catalog does not gain the new exports until the tool is rebuilt/upgraded.
  Nothing is auto-installed.
- Adds candidate-quality and catalog/source/export parity regressions; extends the native IBus
  pack fixture and Linux distribution checks to the new collections. See
  [vocabulary data](data/lexicons/README.md) and [pack usage](docs/linux-packaging-data.md#离线词库包与制包-sdk).
- The travel/work follow-up retains the old meeting-minutes and acceptance-standard continuations,
  adding more specific contexts instead. Independent enable/disable checks keep themes opt-in;
  generated phrases neither read bookings/project records nor perform external actions.
- Adds a private GTK3 per-key gate for the earlier desktop draft-check failure: stable real
  input contexts, exact first/repeated keys, bilingual adoption/undo/commits, language chords,
  field changes and companion focus. CI runs default and forced-sync IBus transport. This
  improves diagnostics and coverage; it does not establish a fix for the GNOME/Wayland failure.
- Completes that gate's tray fixture: an owned StatusNotifierWatcher lets the real panel
  register its menu, and the tenth workflow activates through that menu from a real private
  XKB engine before switching Chinese to English. Menu readiness is observed after one
  AboutToShow, without repeating activation or input. Personal desktop acceptance remains open.
- Fixes a separate tray-activation visibility race: automatic preparation is published when
  the tray accepts activation, before queued refresh/switch work, and never marks the active
  draft as manually hidden. A first native draft arriving before preparation remains visible;
  delayed worker preparation/completion cannot override later explicit Show/Hide. Busy or
  disconnected requests do not prepare visibility.
- Opening settings or editing the seed transfers an automatic popup to explicit visibility;
  empty/unfocused/disconnected native snapshots no longer close that interaction or its window
  after editing finishes. Late native drafts cannot overwrite local text while the window-focus
  event is pending. Native tool tabs still update/auto-hide, and invisible snapshots still clear
  mirrored content. These fixes do not establish the cause of the earlier personal-desktop
  synthetic-key failure.
- Defaults to the custom Suzaku candidate surface once it has actually presented a public
  native frame. A short host/context-bound lease suppresses only IBus lookup/auxiliary UI;
  hidden, folded, occluded or locally edited panels relinquish it. UI-thread heartbeats and
  a 1.2-second host expiry after the last accepted renewal restore system candidates after
  stalls/crashes without another key (up to about 1.8 seconds from a UI stall).
  Passive snapshot subscribers do not suppress candidates, and display acknowledgements never
  change draft revisions, selection, model requests or commits.
- Fills a basic Chinese fallback gap with data-only readings for 强、厉害、很强、很厉害 and four
  authored continuations. Original 2,043 reading ranks are retained; full/separated Pinyin and
  adoption/spacing regressions stay independent of model availability.
- Adds everyday reactions, feelings, replies, meeting updates and rest/meal expressions to the
  default offline fallback. English adds 24 collocations and 48 sentences with just six new
  indexed forms; Chinese adds 64 readings and 24 contexts / 48 continuations. All eleven earlier
  English layers and 2,047 Chinese readings keep their original order and priority. Data-only
  additions remain independent of models and optional packs; index/candidate budgets are unchanged.
- Appends a `daily_needs` batch for food preferences, weather/outings and last-minute plans:
  64 Chinese readings and 24 contexts / 48 sentences per language. English reuses the existing
  index except for three explicit forms (tastes, dessert, road), totaling 6,111 indexed words.
  All twelve earlier English layers and 2,111 Chinese readings retain their ranks; this adds
  authored input expressions, not weather/menu/location services or model-specific behavior.

See the [0.8.1 release notes](docs/releases/0.8.1.md). Historical “unreleased” records below and
in linked audit guides retain their original development context; publication requires same-commit CI.

### 0.8.0 — Offline vocabulary packs and authoring SDK

- Introduces versioned, data-only offline packages independent of model providers. Language/topic
  collections can be browsed/exported, installed, replaced, enabled, disabled and removed through
  `suzaku_tool pack`. Built-ins stay available; conflicting contexts fail safely and damaged startup
  records are isolated. Limits bound file sizes, package counts and per-language indexing work.
- Freezes optional EN/ZH/JA vocabulary once at host/panel startup, keeping file I/O off the input
  path. Changes require restarting both processes; no live reload or settings-panel manager yet.
- Ships English/Chinese outdoors and Japanese rail collections plus a Python standard-library
  authoring SDK. SDK outputs are validated by the Rust tool and published without overwriting.
- Adds `digital`, `home` and `errands` built-in layers: 98 new indexed English forms, 237 Chinese
  readings and 128 / 128 authored English / Chinese sentences. Totals are 6,102 / 2,043 forms/readings
  and 718 / 736 sentences. Existing rank/order fingerprints, candidate and model budgets remain.
- Improves isolated validation on newer GLib/PyGObject: retains borrowed signal payloads until C
  dispatch returns, reuses accessibility observations only within one snapshot, and keeps Compose
  fixture rewrites inside private copies. These are test-harness fixes, not desktop keymap changes.
- Begins the required Linux Release asset policy: publish the exact clean source commit's `.deb`,
  `.tar.gz` and checksums only after that commit's CI and clean-container install checks pass.

See the [0.8.0 release notes](docs/releases/0.8.0.md) and
[pack lifecycle and SDK](docs/linux-packaging-data.md#离线词库包与制包-sdk).

### 0.7.5 — Bilingual sentence continuity and safer English context

- Adds an independent `clarity` vocabulary layer: 58 explicit English forms (70 indexed forms
  including sentence projections), 175 Chinese readings including the additional 再 homophone,
  and 80 authored sentences per language. Totals are 6,004 English forms and 1,806 Chinese readings;
  prior ranks, homophone order, Japanese data and budgets remain protected by existing fingerprints.
- Fixes N58: Chinese authored sentence suggestions retain the full matching progress across
  word adoption and verified Pinyin tails within the existing 256-character local window.
  Same-sound alternatives can supply a sentence without replacing the primary conversion or
  already adopted Han text. Literal spacing and exact adoption/undo/commit behavior remain.
- Fixes N59: English local collocations and sentences no longer cross real line/paragraph
  boundaries or normalize identifiers, domains, digits and clipped context fragments into words.
  Horizontal spacing, case, contractions and valid new phrases after protected text remain usable.
- Expands fallback checks to 44 authored scenarios per language and real GTK vocabulary checks
  to 42 workflows, including Chinese multi-word sentence progress. Native modes each include
  16 actual loopback HTTP 503/deadline cases, three N58 workflows and eight N59 workflows.
  The basic GTK gate adds two physical English boundary/edit-recovery workflows.

See the [0.7.5 release notes](docs/releases/0.7.5.md),
[Chinese sentence-progress audit](docs/bug-audit-sentence-progress-2026-09-27.md) and
[English context audit](docs/bug-audit-english-context-2026-09-28.md).

### 0.7.4 — Reliable model fallback and broader bilingual vocabulary

- Adds three independent fallback vocabulary layers: 658 explicit English forms, 488 Chinese
  readings and 272 short sentences in each language. The English index grows to 5,934 forms;
  Chinese reaches 1,631 readings. Old base ranks, homophone order and Japanese data are preserved.
- Covers everyday clarification, work/travel, time arrangements, shopping/after-sales and common
  verb forms. Local candidates appear immediately with or without a model; this is bounded authored
  fallback, not open-ended generation, a downloaded corpus or runtime dictionary import.
- Fixes N57: a full model response could discard a learned local sentence before applying its
  preference bonus. Ranking now precedes the final cutoff within the existing bounded merge pool;
  final capacity, word/sentence quotas, typing anchors and explicit selection locks stay unchanged.
- Extends offline regression cases to 36 per language, real GTK vocabulary workflows to 36, and
  actual HTTP 503/deadline cases to 12 per native mode. Tests retain literal spacing, adoption undo,
  continued drafts and exact commits without changing the original suite deadlines.

See the [0.7.4 release notes](docs/releases/0.7.4.md),
[fallback vocabulary record](docs/ibus-candidates.md#模型不可用时的本地兜底词库) and
[N57 preference/model audit](docs/bug-audit-preference-model-2026-09-27.md).

### 0.7.3 — Vocabulary resources and in-memory candidate preferences

- Adds two rounds of authored everyday English/Chinese vocabulary: 5,251 indexed English
  forms, 1,143 Chinese readings, 238 English and 256 Chinese sentence examples in total.
  Existing spelling, spacing, literal choices and word/sentence quotas remain protected.
- Moves English, Chinese and Japanese vocabulary into versioned embedded JSON resources.
  The data interface is independent of decoders, native hosts and model providers; parsing
  and indexing happen once per language. This is not runtime import or hot reload.
- Adds a bounded frequency cache independent of dictionary/model implementation. Confirmed
  choices adjust existing IBus candidates; editable adoptions wait for final submission.
  Canceled, undone, private or edited-away choices do not learn. English identifier/word
  boundaries are checked, and asynchronous model refreshes do not stack bonuses.
- Preferences contain salted fingerprints and decaying counts in memory only; the tray can
  clear them, and host restart forgets them. No history file or input-thread persistence is added.
- Adds a real GTK vocabulary workflow suite and connects it to CI. Native preference checks
  cover numeric, Shift+Enter, Space and companion adoption, undo, privacy, focus and clear.

See the [0.7.3 release notes](docs/releases/0.7.3.md), [vocabulary contract](data/lexicons/README.md)
and [candidate preference scope](docs/ibus-candidates.md#偏好频率缓存).

### 0.7.2 — Draft-preserving language shortcuts and responsive settings

- Ctrl+Shift+Space toggles English/Chinese in a normal, focused IBus field, retaining the
  latest draft after persistence succeeds. Empty drafts and both shortcut profiles are supported;
  Japanese switches to English. Held keys remain one-shot, including early modifier release.
- N56 moves settings reads, writes and file sync out of the input loop and shared session lock.
  A single worker stages the change; publication validates the request lifetime, deadline and
  settings baseline. Canceled jobs never publish late, and a busy worker cannot multiply.
- Pending language gestures are canceled by focus, commit, clear, privacy and Compose boundaries.
  Save errors keep the old language and draft. Physical modifier presses preserve pending accents.
  F1 exposes the localized shortcut reference; no global bindings or arbitrary rebinding are added.
- The isolated keyboard gate grows to 49 groups; the controlled slow-I/O gate checks 9 settings
  cases plus 10 keyboard-language cases. Historical release failures are not attributed to this fix.

See the [0.7.2 release notes](docs/releases/0.7.2.md), [shortcut reference](docs/shortcuts.md)
and [N56 settings I/O audit](docs/bug-audit-settings-io-2026-09-27.md).

### 0.7.1 — Activation diagnostics and installation audit

- Private IBus tests preserve real CLI stderr and exit codes on activation/release failures,
  without changing production restoration logic or retrying failures until success. Dedicated
  checks reject desktop access, invalid scope, self-recursion and unrelated commands.
- A guarded repeat-test mode complements the complete native suites. The 0.7.0 CI failure
  did not reproduce in 130 repeated cycles; rerunning the failed job at the same commit passed.
  Its root cause remains unconfirmed, and that older run does not validate the new diagnostics.
- The 0.7.0 installation audit records 124 isolated application checks and 13 real GNOME
  input workflows, plus tray/service quit and reopen. Two desktop cases stopped at the focus
  guard remain incomplete; package metadata and documentation do not update the installed app.

See the [0.7.1 release notes](docs/releases/0.7.1.md),
[activation follow-up](docs/ci-activation-followup-2026-09-27.md) and
[installation audit](docs/install-audit-0.7.0-2026-09-27.md).

### 0.7.0 — Continuous word-to-sentence input

- N54 keeps an adopted Chinese phrase and its horizontal spacing in the bounded local window,
  preserving authored sentence choices before the next Pinyin input. Longest matches retain
  the complete phrase rather than falling back to a shorter suffix. See the
  [round-46 audit](docs/bug-audit-adopted-continuation-2026-09-27.md).
- N55 shares that match with the ordinary IBus mix: adopting Pinyin can shrink a draft from
  above 256 code points back within the short path without losing its sentence. Prefixes,
  whitespace, protected tokens and full-draft model limits remain unchanged. See the
  [round-47 audit](docs/bug-audit-continuation-threshold-2026-09-27.md).
- Bilingual native and real-editor checks cover threshold changes, Space, punctuation deletion,
  exact word/sentence undo and explicit submission. Controlled providers verify full-draft
  requests and stale-result rejection at the model limit; no model quality claim is added.
- The stock-popup fixture no longer assumes exactly nine English candidates. It checks every
  actual slot while still requiring a full first page and a partial last page. This fixes the
  0.6.9 CI fixture failure without reducing click or saved-text coverage.

See the [0.7.0 release notes](docs/releases/0.7.0.md) for verification and upgrade boundaries.
Historical audits retain their original test counts and publication/installation state.

### 0.6.9 — Vocabulary, long drafts and panel ergonomics

- The curated English index grows from 1,242 to 3,906 explicit forms; Chinese Pinyin grows
  from 98 to 602 entries. More authored collocations offer words and sentences together.
  A stable first-letter Pinyin index bounds matching work; ambiguous alternate conversions
  no longer crowd authored/model sentences off the first page. No downloaded corpus,
  personal learning, forced correction or model dependency is introduced.
- English and adopted-Chinese drafts beyond 256 code points retain bounded local tail
  completion, preserving the exact earlier prefix. The existing 8192-byte mirror text and
  65536-byte frame limits remain; fewer alternatives fit very long drafts. Model requests
  still use the unchanged 256-code-point limit. See the [round-45 audit](docs/bug-audit-long-draft-window-2026-09-25.md).
- Settings support visible Tab/arrow navigation, Enter/Space activation, Ctrl+F search and
  scroll-to-focus. Zoom targets are larger and separated, reset uses live scale, and an orb
  press begins WM dragging only after movement. Window titles omit user drafts and debug text.
  See the [installed ergonomics checks](docs/ergonomics-audit-2026-09-25.md); these are not full
  screen-reader or human-comfort acceptance.
- Regression fixtures now compare constant dictionary contents rather than addresses and
  compare runtime-report dispatch against fixed ready/unready states. Development instructions
  distinguish toolchain visibility, required feature flags and short Unix-socket fixture paths.

See the [0.6.9 release notes](docs/releases/0.6.9.md) for verification and upgrade boundaries.
Historical reports retain their original test counts and installation state; source publication
does not update the installed application or establish remote CI/package success.

### 0.6.8 — Bilingual candidate continuity and Linux QA

Includes audit rounds 40–44 and N50–N53. See the [0.6.8 release notes](docs/releases/0.6.8.md)
for validation and upgrade scope; a source tag does not update the installed application.

#### Strict real-editor save synchronization

- The 0.6.7 Linux CI stopped at a popup saved-document assertion. Local repetition also exposed
  a cleared editor buffer with old file contents. File bytes can change before the asynchronous
  editor save has finished; a following Ctrl+S may encounter a disabled Save action.
- Real GTK checks now observe only their owned editor PID/document and require exact buffer text,
  an idle save indicator and exact file contents. Only disposable editor preferences postpone its
  periodic draft autosave beyond the unchanged 240-second gate, avoiding a disabled Save action.
  Editor-only GTK animations are disabled so readiness checks do not wait for decorative fades.
  Each save still uses one real key chord, with no repeated input or relaxed text assertions.
  Fourteen deterministic barrier/scope checks and a real two-editor observation gate are in CI.
- This changes test synchronization, not Suzaku input routing, save behavior in personal applications
  or keyboard mappings. See the [round-40 audit](docs/bug-audit-editor-save-2026-09-24.md).

#### One-word English continuations before Space

- N50 classifies a single next word after a known complete English word consistently with the
  same continuation after a typed Space. `hello → hello world` is a word continuation, while
  longer phrases and punctuated sentences remain sentence candidates. Literal anchors stay put.
- Sentence-only model replies can now also supply that next-word choice before Space, using an
  exact prefix of the returned text and retaining the full sentence. Unknown/incomplete readings,
  identifiers, URLs and hyphenated tails are not split into invented word choices.
- The change is provider-independent and does not add vocabulary, auto-correction, automatic
  submission or new keyboard bindings. Chinese/Japanese algorithms remain unchanged. See the
  [round-41 audit](docs/bug-audit-english-next-word-2026-09-24.md).

#### Pinyin tone numbers at punctuation boundaries

- N51 consumes an optional Pinyin tone number before sentence punctuation or a closing quote/bracket,
  so `ni3hao3，shi4jie4！` converts to `你好，世界！` without leaking tone digits into the text.
  Punctuation and the exact raw-spelling candidate remain available; no automatic commit is added.
- Decimal/time separators followed by alphanumerics and numeric grouping commas remain literal,
  as do multi-digit numbers, fractions, operators and digits outside the existing 1–5 tone range.
  The current vocabulary and English/Japanese algorithms are unchanged. See the
  [round-42 audit](docs/bug-audit-pinyin-tone-punctuation-2026-09-24.md).

#### Exact local prefixes in model continuations

- N52 preserves the whole known CJK local-conversion prefix in provider validation and in both
  standalone/native candidate merging. Indentation, repeated spaces and Unicode horizontal spaces
  no longer disappear from a correctly prefixed Chinese model reply or break its bounded length check.
- Built-in HTTP providers reject prefix-losing replies instead of silently accepting trimmed text.
  English behavior, unconverted phonetic input, candidate budgets, model permissions and cancellation
  stay unchanged. Both protocols and synthetic native adoption/undo/Space/Enter flows are covered.
  See the [round-43 audit](docs/bug-audit-model-prefix-spacing-2026-09-25.md).

#### Pinyin continuation after long adopted drafts

- N53 fixes the offline decoder's 128-round cutoff within its existing 256-code-point input
  limit. Long literal prefixes and repeated syllables no longer hide the exact conversion or
  promote a guessed extension ahead of it. The bounded input determines the search rounds;
  beam width, vocabulary, ranking and maximum input length stay unchanged.
- Long Chinese/English adoption, exact undo, Space continuation and commit checks now cover
  both native entry routes and real GTK editor output. Shortening a 257-character draft restores
  conversion at 256; see the [round-44 audit](docs/bug-audit-long-pinyin-continuation-2026-09-25.md).

### 0.6.7 — Context-local shortcuts and literal boundaries in Chinese drafts

- A searchable Shortcuts settings page preserves the standard layout and offers opt-in QWERTY
  home-row IBus aliases: Alt+J/K navigate, Alt+H/L page, Alt+semicolon adopts without submitting.
  Focus-local F1 opens the reference; Ctrl+comma opens panel settings. No global key grabs.
- Shortcut preferences use acknowledged narrow native patches and configuration backups, with
  failure/conflict protection. Changing only the layout preserves input, selection, undo, Compose
  and prediction. Modifier/privacy/idle guards preserve application keys. See [shortcuts](docs/shortcuts.md).
- N49 makes physical candidate-adoption holds one-shot until release, retaining exact undo and
  immediate separate taps. Releasing a modifier first cannot turn held adoption into submission;
  navigation/text editing still repeat. See the [repeat audit](docs/bug-audit-shortcut-repeat-2026-09-24.md).
- Physical keyboard coverage now includes releasing selection keys in another real editor window,
  reusing the same key, exact undo and returning focus without replaying the old draft. The gate
  has 44 workflows; see the [focus follow-up](docs/bug-audit-shortcut-focus-2026-09-24.md).

- N48 keeps literal whitespace, quotes and non-Pinyin Unicode case around converted/adopted text.
  Only actual phonetic separators are consumed; line/paragraph boundaries are never syllable joins.
  Existing `ü`/`u:` and tone forms, `xi'an` disambiguation and literal fallback remain available.
- Word/sentence continuations retain literal horizontal padding. New paired native checks cover
  owned-prefix keyboard continuation and companion replacement, adoption/undo and exact submission.
  English input is a regression control; its algorithm and dictionary are unchanged.
  See the [literal-boundary audit](docs/bug-audit-chinese-literal-boundaries-2026-09-24.md).
  See the [0.6.7 release notes](docs/releases/0.6.7.md) for validation and upgrade boundaries.

### 0.6.6 — Bilingual input and model responsiveness

- Existing Chinese entries now match joined or correctly separated Pinyin, including unfinished
  readings, while preserving syllable boundaries and literal fallback. Paired Chinese/English
  quality suites and native word/sentence adoption, undo and continuation checks guard the core flow.
- Newer drafts cooperatively cancel obsolete connected local HTTP candidate/discovery requests;
  a single worker and the original total deadline remain. Blocking cloud/legacy calls still
  finish or time out before discarding results. Translation cancellation is unchanged.
- Compact, model-independent candidate prompts request two complete strings and reject request-field
  echoes. Translation restates its task and rejects lost question marks without changing the draft.
  Real CPU LLaMA still has default-budget timeouts and semantic limitations.
- Added isolated physical-keyboard/layout checks to CI and Firefox, VS Code and opt-in real-model
  workflows. These do not qualify hardware LEDs, native Wayland or every browser/editor.
  See the [0.6.6 release notes](docs/releases/0.6.6.md) for validation and release boundaries.

### 0.6.5 — Service retries and verified input-source recovery

- Newly registered host units retry every two seconds without exhausting systemd's start quota.
  Explicit stop still cancels retries; existing units need explicit re-registration to update.
- A panel owning the registered desktop host observes same-bus host replacement and can restore
  only a confirmed missing global engine. Newer valid choices win; IBus disconnection discards
  observation history. No application text is monitored and no old draft is replayed.
- Password diagnostics reset their own input purpose and destroy their synthetic context before
  bounded, read-back-verified restoration. Missing fallback input sources fail before switching.
- Added private daemon-loss, real isolated systemd and input-source recovery regressions. Explicitly
  authorized GNOME 46/X11 installation checks also pass with the same fixes before the version bump.
  See the [0.6.5 release notes](docs/releases/0.6.5.md) for validation and upgrade boundaries.

### 0.6.4 — Smaller build caches and test fixture cleanup

- Development/test profiles omit debug symbols and disable incremental compilation, matching CI.
  Ordinary dependency caching, assertions and overflow checks remain; release settings are unchanged.
- Theme-setting tests remove their freshly owned temporary directories when the fixture drops,
  including during unwinding. A regression verifies cleanup; personal settings are never targeted.
- Kotlin's local cache is ignored, and the contribution guide documents scoped cleanup and debugger
  overrides. No automatic user-cache deletion or input-rule change is introduced.
  See the [0.6.4 release notes](docs/releases/0.6.4.md).

### 0.6.3 — Bounded popup previews and clickable ordinals

- Candidate-area draft previews keep a suffix within 48 display cells and 160 Unicode code points,
  preventing tested unwrapped stock popups from extending beyond the screen. Drafts remain lossless.
- Superscript shortcuts now share the candidate text's click target, avoiding IBus 1.5.29's invalid
  separate-label indices without changing system files, candidate payloads or number-key adoption.
- Strict popup coverage includes 46 workflows, with 17 actual ordinal-glyph checks; another 21
  real GTK lifecycle workflows cover switching, privacy and process reconnection.
  See the [0.6.3 release notes](docs/releases/0.6.3.md).

### 0.6.2 — Candidate-area drafts and literal numeric input

- Default drafts stay in the candidate area/companion, not the application's preedit cache.
  Tested Chrome/Qt fields no longer commit them on focus changes, and cancelling a replacement
  preserves the original selection. Enter/candidate clicks remain explicit submission.
- With no draft, ordinary/keypad digits go directly to the application. Within a draft, number
  choices and Alt+digits retain their existing behavior; Space still continues writing.
- Opt-in inline preedit remains available, with complete empty updates for synchronous IBus
  cleanup. It retains the client's implicit-confirmation risk on focus changes.
- Real GTK/Chrome/Qt workflows and Unicode/long-draft boundaries have isolated regression coverage.
  See the [0.6.2 release notes](docs/releases/0.6.2.md).

### 0.6.1 — Input-target boundaries and safe continuous typing

- Independent panel sends cannot inject into PRIVATE fields. Privacy/bypass transitions invalidate
  old input targets, including brief round trips hidden by coalesced snapshots.
- Actual language changes and whole-draft cancellation reject delayed old sends and stale presses;
  same-language settings, Compose-only cancellation and ordinary editing remain continuous.
- Physical Enter, native candidate clicks and independent text sends end old input work after a
  successful commit. Revision-bound companion commits keep their acknowledged follow-up typing.
- Candidate-commit history remains exact within the same field; late model replies cannot restore
  canceled or committed drafts. See the [0.6.1 release notes](docs/releases/0.6.1.md).

### 0.6.0 — Native input state and exact model context

- Reselecting the current language preserves drafts, completion undo and pending Compose sequences.
- Repeating the current prediction switch or Tone preserves candidate selection and model results;
  configuration conflict checks, failed-save protection and explicit reloads remain enforced.
- Destroying the focused IBus engine without FocusOut ends its session and clears companion state.
  Late destruction or model results cannot affect a newer field.
- Native model context concatenates the exact candidate payloads, without the independent editor's
  automatic separators. Application output, editor spacing and continuous-drafting keys are unchanged.
  See the [0.6.0 release notes](docs/releases/0.6.0.md).

### 0.5.9 — Translation gestures and continuous candidate input

- Translation requests require a fresh gesture after the source, field or configuration changes;
  cancelling also invalidates a press that has not started a request yet.
- Screen-keyboard input after acknowledged tool/candidate operations starts from the resulting draft
  and waits for its authoritative revision. Newer local work and target/privacy boundaries remain protected.
- Intermediate empty snapshots no longer hide a companion with queued input or forget manual hiding.
- Selecting unchanged literal text publishes the new selection to IBus and the companion without
  rebuilding candidates or committing. See the [0.5.9 release notes](docs/releases/0.5.9.md).

### 0.5.8 — Lifecycle recovery and input-target boundaries

- Quit retries retain unfinished input-method restoration and queued host-start cleanup.
- Failed single-instance handoffs no longer open an unguarded extra panel. Coalesced native
  updates can wake the companion for a new field without overriding manual hiding in the old field.
- Voice/handwriting adoption gestures are cancelled when their native input target changes;
  source material remains available for a fresh click. Continuous drafting and explicit submission
  rules are unchanged. See the [0.5.8 release notes](docs/releases/0.5.8.md).

### 0.5.7 — Continuous input and Linux reliability

Completion/undo, tool insertion, settings recovery, confirmed model configuration, data restore
locks and Linux registration/packaging were hardened. See the [0.5.7 release notes](docs/releases/0.5.7.md).

### 0.5.6 — English completion and input reliability

Improved English completion, Compose/dead-key handling and native editing recovery, with functional
network audits and strict Clippy coverage. See the [0.5.6 release notes](docs/releases/0.5.6.md).

### 0.5.5 — Draft translation, clearer text and compact settings

- Declared numeric/decimal/phone fields bypass candidate shortcuts and panel injection.
  Screen-keyboard bursts retain unsent edits behind one acknowledged write; stable letter
  presses survive candidate refreshes. Uncertain writes never automatically replay across fields.
- English word/sentence continuation shares authored collocations through spaces and partial
  words. Long previews show the differing suffix without changing payloads; Ctrl+Backspace
  deletes a whitespace-delimited English draft word.
- Linux packaging declares keyboard runtime and Latin/CJK font dependencies. Clean Ubuntu
  container checks cover install/upgrade/remove; registration checks cover special paths and safety.

- **Settings → Appearance → Interface** selects the application's display language independently
  of input and translation languages. English, Simplified Chinese, Japanese, Korean, Spanish,
  French, German and Portuguese are bundled offline; **System** follows the locale environment
  with English fallback. Changes update panel/settings/tray immediately and survive restarts and
  settings backups. Search accepts translated and English labels; narrow layouts reflow longer
  labels. Drafts, candidates, model results and provider consent are unchanged. Raw system/service
  diagnostics retain their original wording. See [interface languages](docs/interface-languages.md).

- The panel's **文 / Translation** tool translates the current draft between English, Simplified
  Chinese, Japanese, Korean, Spanish, French, German and Portuguese. Choose **From / To**, then
  **Translate**; **Auto** detects the source. Read longer output with the page arrows and use
  **Use draft** to replace the editable composition, never to send it automatically.
  Translation uses the configured local/cloud provider with a separate 30-second deadline;
  cloud consent remains required. Source text, translations and navigation are session-only.
  Editing the source, changing language/context or reloading model settings discards stale results.
  See [translation behavior and limits](docs/translation.md).

- Settings are grouped into **Appearance**, **Input** and **Model** tabs. Search spans every tab;
  selecting a tab clears search and resets scrolling. Labels/options share aligned columns, the
  search field uses the available width, and the window fits the current page's content height
  while retaining scrolling on smaller screens. Category changes never modify saved IME options.

- **Settings → Appearance → Title Bar → Hide** removes the desktop window title bar from the panel and
  settings immediately; **Show** restores it. Suzaku's own drag regions and close buttons stay
  available. The choice survives restarts, settings backups and orb expansion. Existing profiles
  keep their title bars until changed; no GNOME-wide settings are modified.
- Borderless panels and settings now have transparent exteriors instead of a square pale
  backplate. Rounded cards retain their subtle shadow and unchanged controls. Both windows request
  alpha-capable native surfaces, including Linux/XWayland's inherited-alpha path; opaque-only
  drivers fall back to the theme background. Showing the system title bar restores its backdrop.
  On hybrid-GPU Linux laptops, an alpha-capable hardware adapter is preferred over an opaque-only
  adapter, with fallback if device initialization fails; software rendering is not forced for this.

- Panel and settings text now rasterizes at the glyph's physical display height, rather than
  shrinking a shared large bitmap. Pixel-aligned ink retains its native width and antialiasing;
  fractional layout advances and clipping stay intact, including at 90% zoom and higher DPI.
- A bounded, padded atlas caches multiple sizes of each character without stretching narrow
  letters or mixing neighboring glyphs. Zoom and raster eviction retain resolved fonts and stable
  layout measurements. Saved font, smoothing and zoom choices are preserved; system IBus popup
  rendering and global font settings are not changed.

### 0.5.4 Guardian themes, continuous input and service lifecycle

- Linux panel startup now starts the registered `suzaku-ibus.service` and waits for its IPC
  readiness without activating Suzaku. Activation retries a stopped host automatically. A full
  quit restores the observed previous input method before stopping the host; hide-to-tray keeps
  it running. Startup/shutdown run on the background controller, including desktops without a tray
  extension. Restore/stop failures remain visible and retryable; a disconnected host is no longer
  mislabeled as an outdated version. Custom/private endpoints remain owned by their own launcher.
- Native IBus now uses one continuous writing stream: plain **1–6** adopts the current page's
  candidate without committing, **Alt+digits** enters numbers, and Space/punctuation stay in the
  draft. Enter or a primary candidate click submits it. Shift+number symbols retain the active
  keyboard layout. English, Pinyin and Romaji share the same rules; both local and model
  predictions see the complete uncommitted draft. Panel candidate shortcuts also keep text editable.
- New default **Suzaku** theme: warm ivory surfaces, vermilion accents, soft gold details and
  quieter candidate highlights. Choose **Settings → Theme → Suzaku** for an existing profile;
  explicitly saved themes are retained. The native companion and backup format recognize `suzaku`.
- Three additional styles under **Settings → Theme**: **白虎 · 米白** (warm ivory / graphite),
  **青龙 · 青紫** (misty jade / violet), and **玄武 · 靛蓝** (deep indigo / silver blue).
  They cover the panel, candidate highlights, keyboard, handwriting, dictation, settings and
  tooltips. Saved IDs are `baihu`, `qinglong`, and `xuanwu`; settings reloads and backups retain them.
  The orb and Linux tray use matching curved tiger, dragon, and tortoise/serpent emblems. Tray
  icons update only when the theme changes, on the background worker, not on input/render frames.
- Rounded cards and the redesigned toolbar icons use resolution-independent, antialiased curves
  and strokes. The default floating orb and Linux tray share a swept-feather phoenix emblem; tray sizes
  are supersampled individually. The orb has transparent surroundings when the compositor and
  graphics surface support premultiplied alpha, with a themed background fallback otherwise.
- New profiles use the system Auto font and Smooth text. Loading settings no longer silently
  replaces Auto/Smooth with Monaco/Sharp. Text layout, centering and the caret use cached system
  glyph widths, so narrow letters are no longer stretched across a fixed character cell.
- Existing candidate actions, non-functional drag regions, close-to-tray behavior and content-fit
  resizing are retained. Desktop panel styling does not override GNOME's system IBus popup theme.
- Bug fixes: the input row preserves repeated spaces and scrolls long drafts with the caret,
  instead of wrapping over controls or eliding editable text. Its height follows text size.
  Fractional zoom keeps glyph geometry intact, Smooth/Sharp changes the active sampling filter,
  and new tooltip/feedback glyphs are measured before the first frame is displayed. Single-symbol
  controls fit their real glyph width so the zoom `+` cannot turn into an ellipsis at Large text size.
- Model reloads preserve the current IBus draft while a provider change forgets previous committed
  context. English model candidates retain typed indentation, spacing and literal list prefixes
  through parsing, ranking and commit. A missing/disconnected local model invalidates discovery
  for the next request instead of keeping a stale success entry for 60 seconds; no failed generation
  is automatically retried.
- Output fixes: consecutive native commits preserve each candidate's exact leading/trailing
  spaces. Standalone Linux panel sends use the displayed candidate, not another bridge's candidate or
  cumulative history. Only acknowledged delivery clears the editable draft and records a local
  commit; rejected/uncertain sends retain it with an explicit warning and are never retried automatically.
- IBus focus boundaries now reject late key, navigation and candidate-click events from unfocused
  engine objects. Focus-in clears the previous field's preedit, undo and prediction context even
  when focus-out is delayed. Native candidates commit only on primary clicks without application
  modifiers; physical Mod4 and virtual Super/Meta/Hyper shortcuts pass through without changing input.
- Linux configuration loading opens nonblocking and validates the opened file type before reading.
  FIFOs (including symlink targets), sockets and directories fail without freezing native input;
  failed reloads preserve the current settings and draft. Symlinks to regular JSON files still load.
  The real activation/input/release regression now checks six-row English/Chinese/Japanese
  candidates and runs in CI only under a guarded private IBus session.

### 0.5.3 Model services and IBus candidate experience

Model identity, local/cloud scope and API protocol are now independent. Local discovery prefers
an installed LLaMA through fixed loopback metadata endpoints; explicitly configured HTTPS cloud
services require opt-in consent. Credentials are read from a named environment variable, never
saved in settings/backups; restoring a backup revokes cloud consent. Legacy Llama configuration
and command aliases remain compatible. See [model provider configuration](docs/model-providers.md).

IBus now mixes alphabet/Pinyin/Romaji word completions with short phrase/sentence candidates.
There are six rows per page and at most twelve candidates. Superscripts distinguish words (`ᵂ`),
sentences/phrases (`ˢ`), literal input (`ᴿ`) and model suggestions (`ᴬᴵ`); subscripts show bounded
ranking weights, not confidence percentages. The first page reserves room for both types when
available, while preserving the literal/best-offline typing anchors. Offline collocations work
without a model; enabled local or cloud providers can add typed word and sentence suggestions.
See [IBus candidate UX and keyboard controls](docs/ibus-candidates.md).

Continuous CJK input now completes an unfinished final word without losing the converted prefix
(`woxihuanbei` → `我喜欢北京`, `watashihanihong` → `私は日本語`). Pinyin spaces preserve
syllable boundaries (`xi an` / `xi'an` → `西安`, distinct from `xian`).

Shift+Enter adopts a full candidate into editable preedit; Space continues from an explicitly
selected candidate. Immediate Backspace restores the previous spelling. This one-step undo stays
in memory and is cleared at editing, reset, language, privacy and focus boundaries. No candidate
annotation or truncated preview enters the committed text.

### 0.5.2 Linux packaging and data management

- Build binary `.tar.gz` and Debian/Ubuntu `.deb` packages with
  `bash scripts/package-linux.sh`. Packages include the four Linux programs, desktop launcher
  (for `.deb`), manifest, checksums and dependency license inventory. Installation never starts
  a service, registers an input source or downloads a model automatically.
- Tray **数据管理** provides configuration backup and folder shortcuts. `suzaku-tool data`
  supports status, backup, validation and preview-first restore. Linux restore requires stopped
  hosts/panel, saves a before-restore backup and preserves unrelated user files.
- Existing configuration locations are retained; new configuration writes are private and atomic.
  No input history, drafts, caches or external model weights are collected.

See [Linux packaging and data management](docs/linux-packaging-data.md) for installation,
upgrade/uninstall, path conventions, restore commands and compatibility limits.

### 0.5.1 Patch fixes

- Preserve literal English digits and preedit across modifier/function keys. Keep CJK numeric
  candidate selection, and use absolute candidate navigation for later-page AI probe results.
- Prevent stale candidate presses and duplicate mouse/touch completions. Synchronize Tone through
  acknowledged native settings, preserve custom temperatures, and retain decimal/version prefixes
  in Llama responses. Use a process-lifetime lock for concurrent panel startup and crash recovery.
- Keep Linux/IBus IPC reads and replies off the blocking path with main-loop socket sources.
  Incomplete requests have a one-second absolute deadline, a 65,535-byte limit, and a cap of
  16 pending clients. Oversized, invalid UTF-8 and incomplete requests cannot commit a prefix;
  legacy commits are rejected if focus changes before the complete request arrives.
- Refresh Linux panel capability/status checks in one on-demand background worker. Startup uses
  a conservative snapshot; an expired three-second cache returns its last result immediately.
  External status commands have a 350 ms deadline and 64 KiB output limit; timed-out probe
  process groups are stopped and the direct child is reaped. Diagnostic APIs remain synchronous
  with these bounds; UI input handlers never wait for a status subprocess.
- Apply one total deadline to Linux client connections, partial writes and responses: 200 ms
  for subscription setup/each read turn, 350 ms for native actions, availability checks and legacy
  text output, and 700 ms for settings. A saturated accept queue cannot block indefinitely;
  trickled responses cannot reset the budget. Timed-out operations are never replayed.
- Reuse the bounded command runner for tray IBus operations, including output collection, with
  a two-second deadline and 64 KiB output limit. Inherited stdout cannot extend the deadline;
  the unreaped direct child reserves its process-group identity until cleanup. Failed/uncertain
  switches retain their restore target. Native sync workers support cancellation on panel exit,
  and partial subscription frames yield without recursion or discarding their buffered bytes.
- Keep voice transcripts and handwriting drafts until a revision-bound native replacement is
  acknowledged. Busy, rejected, disconnected or timed-out requests retain the source for explicit
  retry, without falling back to another target. Late acknowledgements cannot clear newer drafts.
- Wait for 900 ms without a transcript change before automatic voice insertion, including backends
  that emit each update only once. Changed partials restart that interval; failed handoffs stop
  capture and never auto-retry. This is a quiet-time heuristic, not a final-utterance signal.
- Preserve leading/trailing spaces, tabs and newlines in fallback text output. Blank-only input is
  still rejected and embedded NUL bytes are still replaced by spaces.
- Remove macOS speech file logging, including transcripts and framework error descriptions.
  This prevents new logs; pre-existing logs are not removed. macOS runtime verification is pending.

### 0.5.0 Highlights

- Added shared English, Chinese and Japanese input profiles, with English-first offline word and
  phrase completion, language-aware commit spacing, and persisted settings.
- Added opt-in asynchronous local Llama candidates through Ollama or compatible local servers,
  with configuration, status, warmup and probe tools; offline candidates remain available.
- Completed Linux/IBus tray activation and restoration, plus real-time native composition,
  candidate and selection synchronization with the non-focusing panel. Revision-checked panel
  actions prevent stale commits, and private fields do not expose input to the panel.
- Restored direct panel keyboard editing, including IME preedit/commit events and focus return,
  while keeping candidate clicks and background dragging non-focusing.
- Fixed multilingual glyph fallback, candidate visibility, handwriting overlap, control alignment,
  background dragging, and content-fit resizing through tab folding, zoom and compact mode.

Native input synchronization is currently implemented and exercised on Linux/IBus. The shared
language framework is portable; other native host adapters and larger CJK dictionaries remain
follow-up work. See verification and known limitations below.

### 0.5.0 IME foundation

- Shared Chinese, English and Japanese language profiles with language-specific conversion and
  commit spacing; the former English demonstration sentences have been removed.
- Immediate offline candidates plus debounced, asynchronous local-LLM enrichment in the native IBus
  host and desktop companion. Late responses cannot replace a newer composition or a selected list.
- Tray language selection, local-LLM opt-in, model configuration reload, and persisted IME settings.
- A literal fallback stays available, AI candidates are marked, and only explicit selection commits
  generated text. In native IBus input, password/PIN fields bypass the engine and private fields use
  offline conversion only; the companion commit channel also refuses password/PIN targets.

This is the first functional foundation, not full dictionary coverage. Chinese currently has a small
bootstrap Pinyin vocabulary and bounded phrase segmentation; Japanese has Romaji/Kana conversion
and a small Kanji vocabulary. Large dictionaries, learned user vocabulary, reliable long-sentence
conversion and automatic next-word suggestions after commit remain follow-up work. Language
profiles can be extended independently of the LLM provider and platform host.

### Using multilingual model candidates (local or cloud)

After building/installing the native host, right-click the Suzaku tray icon and select **输入语言**:

- **中文（简体拼音）**: e.g. `nihao` → `你好`, `shurufa` → `输入法`.
- **English**: literal input first, then word/phrase completions; Space keeps the writing stream editable.
- **日本語（ローマ字）**: e.g. `nihongo` → `日本語` / `にほんご` / `ニホンゴ`.

English is the default for new settings; saved Chinese/Japanese choices are retained. The panel
starts with an empty composition instead of a Chinese demo phrase. The project-authored offline
English baseline has over 900 common words, contractions and computing terms in a cached prefix
index, with explicit collocations for context-sensitive ranking. It is not a comprehensive spelling
dictionary or automatic correction system: literal input remains first and unknown input is kept.

Examples: `hel` → `hello` / `help`, `please sen` → `please send`, `good m` → `good morning`,
and `thank you ` → `thank you for` / `thank you very`. Typed case and spacing are preserved;
URLs, paths and identifiers are not split into word completions. Same-session committed words can
rank the next prefix (`good` followed by `m` prefers `morning`); this context is cleared on focus
loss and private fields do not retain it. An empty preedit does not yet open next-word candidates.

Panel word chips complete the current word or append only the immediate next word. For example,
clicking `hello` after `hel` produces `hello`, never `hel hello`; clicking `world` then gives
`hello world`. The back action reverses each exact completion. Single-word full candidates remain
visible for immediate commit; no arbitrary `is/can/will` words fill an empty live candidate list.
Mouse and touch completion/rewind use one repeat guard. If a model response replaces the
candidate list during a press, releasing that old press cannot select or commit its replacement;
refreshing an unchanged list still permits the click.
On native IBus, **1–6** adopts the current page's full candidate into editable preedit in all three
languages. **Tab / Shift+Tab** navigates, and **PageUp/PageDown** changes pages. **Space** keeps
composing: it adopts an explicitly selected candidate and appends a space, or simply preserves the
literal spelling and adds a separator. **Shift+Space** remains an alias. For example,
`hel` → `2` → Space → `world!` stays one editable `hello world!` draft; only **Enter** or a candidate
left-click commits it. Punctuation does not implicitly end the writing stream.
Use **Alt+0–9** or keypad digits for literal numbers such as `v123` and `2026`; this preserves
Shift+number punctuation (`! @ #`, etc.) and the active keyboard layout. Plain number keys select
while candidates exist; empty slots (including unassigned `0/7/8/9`) do nothing. Without a draft,
digits can start literal input. Neither this mapping nor continuation modifies system lock state.
**Shift+Enter** adopts the current full candidate into preedit without adding a space or committing.
An immediate **Backspace** undoes the replacement and restores the exact previous spelling;
ordinary editing, another selection, reset, language/privacy changes and focus loss clear this
one-step, memory-only undo. AI sentence previews also expand to their full editable text.
Shift, Caps Lock and other non-text keys do not submit pending input. AltGr and system shortcuts
remain separate from candidate selection; password/PIN input still bypasses Suzaku entirely.

Enable **LLM 联想** to enrich the current composition. Offline candidates remain usable
immediately; the local first candidate never changes under Space. The model gets a structured
language ID, raw composition, local conversion, and up to 160 characters committed in the current
focused session. Context is cleared on focus loss and never written to settings. No surrounding
desktop text is collected. Local discovery is the default; cloud input requires an explicit HTTPS
endpoint, model and separate consent. Private input fields never request model candidates.
The former synchronous `IME_NEXT_TOKEN_MODEL_*` environment-variable path has been removed;
use the shared settings and LLM switch below. Panel refreshes never make their own model requests,
and Chinese/Japanese previews do not insert English next-word fillers.
Outside the native mirrored view, sending a standalone Linux panel candidate clears its draft only after
the target acknowledges delivery. Rejected or uncertain output leaves the draft and prior committed
context intact; check the target before an explicit retry, since a lost reply cannot prove non-delivery.

On Linux the model configuration is `$XDG_CONFIG_HOME/suzaku-ime/settings.json`, defaulting to
`~/.config/suzaku-ime/settings.json`. Tray changes create it automatically. Example:

```json
{
  "language": "en",
  "llm_enabled": false,
  "llm_scope": "local",
  "llm_protocol": "auto",
  "llm_endpoint": "http://127.0.0.1:11434/api/chat",
  "llm_model": "auto",
  "llm_api_key_env": null,
  "llm_cloud_consent": false,
  "llm_timeout_ms": 1200,
  "llm_temperature_tenths": 4
}
```

The provider is model-agnostic: model identity, local/cloud scope and API protocol are independent.
Default `auto` discovery prefers an installed **LLaMA** without pinning a particular release or size;
other local model families can also be selected. The native Ollama `/api/chat` integration
uses a bounded typed JSON candidate schema (up to three words and three short sentences),
a 256-token output budget, a 2,048-token context, and a five-minute idle residency.
Existing `/v1/chat/completions` configurations remain compatible, including local llama.cpp servers.
English completions and already-converted CJK phrases keep their local prefix; unrelated output,
unchanged input and pronunciation-only replacements are filtered out. Unknown Pinyin/Romaji can
still be converted by the model without being forced to keep the raw Latin prefix. These guards
do not judge semantic correctness; small-model language quality still needs broader evaluation.
English candidates preserve the exact typed prefix, including indentation, repeated spaces and
literal list markers; response cleanup only removes padding outside that prefix.
English model requests distinguish incomplete-word completion from next-word continuation, and
reject outputs that merely append words to an unfinished fragment. The literal and best offline
English completion keep their positions when AI results arrive; selecting a candidate freezes
the list until the next edit. `suzaku_tool llama probe en` exercises five fixed English examples.
Leave `llm_model` as `auto` to discover local models, or set an **already installed** model explicitly.
Start its service separately. Existing saved model names are preserved, not silently migrated to auto.
Suzaku never downloads a model during typing or starts a model server implicitly.
Choose **重新加载模型配置** after editing. `SUZAKU_IME_CONFIG` can select an isolated settings file
for development. An absent, slow or invalid model response leaves offline candidates available.
Reloading preserves the current IBus composition unless the language changes. Changing the model,
endpoint, protocol, scope or credential reference clears earlier committed context and pending
prediction results, so the new provider starts with only the current draft.
The settings window's **Tone** updates the native Linux host and its saved configuration:
Focused = 0.2, Balanced = 0.4, Expressive = 0.7. Both windows follow acknowledged values;
failed writes roll back, and late replies do not overwrite a newer selection. Reloaded custom
temperatures remain exact and appear as **Custom**, rather than being rounded to a preset.
Tone changes preserve the current composition and do not change the model or language.
The compatible API's line parser preserves decimal/version prefixes such as `3.14` and `1.2.3`.

The tray provides **发现 / 检查本机模型** and **预热本机 Ollama 模型**. Checking only reads metadata;
preheating sends an empty request, not typed text, and runs independently of input-method switching.
The configured service, missing model, timeout, malformed response, and empty candidates are
distinguishable instead of silently appearing as one generic failure. Status reports describe the
last explicit check, not continuous monitoring.

```sh
# Install/start Ollama separately first; keep it local-only (OLLAMA_NO_CLOUD=1).
cargo build --release --all-features --bin suzaku_tool
target/release/suzaku_tool model configure   # local auto; preserves language and opt-in
target/release/suzaku_tool model discover    # metadata only; no downloads or warmup
target/release/suzaku_tool model status
target/release/suzaku_tool model warmup      # Ollama only; empty request, bounded to 30 seconds
target/release/suzaku_tool model probe all   # fixed zh-Hans/en/ja examples, not desktop input
```

`model configure --model NAME --endpoint URL --timeout-ms N` updates only the supplied fields;
no-argument `configure` restores local auto discovery and clears cloud credentials/consent references.
Reload from the tray afterward. `llama` remains a command alias. `probe` preheats explicitly configured
Ollama models and reports local-conversion time, model-request time and actual
candidate text. It does not change the active input method or enable LLM input by itself.

Discovery checks only `127.0.0.1:11434` (Ollama), `127.0.0.1:8080` (llama.cpp) and
`127.0.0.1:1234` (compatible desktop servers), using `/api/tags` or `/v1/models`. A custom
endpoint is checked alone. It uses an 800 ms total metadata budget, caches success for 60 seconds
and failure for 5 seconds, and runs on the prediction worker when first needed, never the UI thread.
Local generation failures reporting a missing model/interface (404) or unavailable service discard
the matching success entry; the next request discovers again, without retrying the failed request.
Timeouts, rate limits and cloud failures do not trigger discovery.
It does not scan files/ports, load/download weights, or fall back to cloud. Ollama entries marked
remote are excluded even when an explicit local model alias is configured. Keep Ollama in local-only
mode as an additional runtime safeguard. The inventory describes the service at check time.

For HTTPS services exposing compatible chat completions, see [model configuration](docs/model-providers.md).
The panel now says **Configured service**, not a fixed Llama preset; `settings.json` is authoritative.

Meta's Llama 3.2 model card does not list Chinese or Japanese among officially supported languages;
these profiles retain deterministic local candidates, and model-quality evaluation is still needed.
See the [Llama 3.2 model card](https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct),
[Ollama chat API](https://docs.ollama.com/api/chat), and
[local-only/preload settings](https://docs.ollama.com/faq).

Romaji spelling conventions were checked against the upstream
[Mozc conversion table](https://github.com/google/mozc/blob/master/src/data/preedit/romanji-hiragana.tsv).
Suzaku's current converter and bootstrap vocabulary are independent, limited implementations.

### IME foundation verification

The Linux build is verified with `cargo test --all-features` and release builds using
`--all-features`. Native IBus checks cover all three languages, AI-candidate selection, private
fields, password/PIN bypass, panel commits, and activation/restoration. Model transport checks
include synthetic loopback HTTP fixtures as well as the local real-model smoke test below.

On 2026-09-08, Ollama 0.33.3 with `llama3.2:3b` (Q4_K_M, approximately 2.02 GB on disk)
was exercised on a Linux machine with an RTX 4050 Laptop GPU (6 GB VRAM). The model used a
2,048-token context and approximately 2.32 GB of VRAM. Initial empty-request loading took about
26 seconds; early generation requests hit the 1.2-second timeout and safely kept local candidates.
After warmup, one fixed `probe all` run measured 523 ms (Chinese), 665 ms (English), and 789 ms
(Japanese), with local conversion taking 14–35 microseconds. These are individual observations,
not latency percentiles, end-to-end keypress timings, or a comprehensive quality benchmark.

An isolated real IBus session also successfully selected and committed AI candidates for `nihao`,
`hello`, and `nihongo`, retained local-only candidates in private fields, bypassed password fields,
and restored its previous engine after each probe. The desktop's active `rime` engine and LLM
opt-in remained unchanged. The current feature-enabled regression suite passes 634 tests, with eight
opt-in tests ignored by default. All seven isolated UI/GPU checks pass separately; the live-desktop IME
activation test is intentionally not run. The local service binds loopback only with cloud use disabled;
model residency expires after five idle minutes.

The Linux blocking-path follow-up uses private IBus/Xvfb sessions and a fixed, non-recursive slow
status-command fixture. In one local run, an idle/partial IPC client increased native key handling
to about 953 ms before the fix and about 1–2 ms afterward. With a 1.2-second status-command fixture,
panel focus/edit/selection handlers returned in about 0.4–0.8 ms, including cache expiry. These are
synthetic regression observations, not latency percentiles or whole-desktop performance claims.
Transport checks also cover fragmented Unicode, exact/oversized frames, trickling clients, focus
changes before a legacy commit, abandoned replies, and bounded pending-client/descriptor cleanup.

The client/tray follow-up also reproduces a full Unix accept queue and inherited stdout without
using the desktop host. The corrected subscription/action/settings/availability calls returned at
about 200/350/700/350 ms, a trickled settings reply stopped at 700 ms, and the tray command that
previously waited three seconds stopped at its two-second limit. Isolated native-sync shutdown
checks cancelled and joined both workers in about 0.6–24 ms, including connected peers withholding
frames/acknowledgements. These timings describe the fixed fixtures, not whole-application shutdown
or general performance guarantees; IME restoration on quit remains a separate bounded operation.

The English-completion follow-up verified native Tab + Space commits for `hel`, `HEL`, `sched`,
`compati` and `don'`, alongside lossless literal commits and Chinese/Japanese privacy checks.
Five fixed English Llama examples returned useful-prefix candidates in 225–461 ms in one warmed
run. This is a transport/interaction smoke test, not a grammatical-accuracy benchmark. GPU readback
also covers English word chips and equal-width collapsed word cards without short-word truncation.

Known repository-wide follow-ups remain: strict `cargo clippy --all-features --all-targets -- -D warnings`
is not clean (including existing GPU/FFI diagnostics), and no-GPU/default-feature builds still have
UI-type/feature coupling. Use the feature-enabled commands above for this native/desktop build.

### Continuous integration

[GitHub Actions CI](https://github.com/chiharu-kiryu/Suzaku-map/actions/workflows/ci.yml)
runs on pushes to `main`, pull requests targeting `main`, and manual dispatches. It uses Rust
1.95.0 and the checked-in lockfile, with read-only repository permissions and pinned action commits.
New commits cancel superseded runs on the same branch or pull request.

- **Rust formatting** checks the entire workspace with `cargo fmt --all --check`.
- **Linux** runs the complete feature-enabled test suite serially, then exercises real IBus
  composition/candidate synchronization in a private D-Bus session. Separate Xvfb processes test
  keyboard focus, mouse/touch candidate completion and async-refresh races, content-fit
  resizing/dragging, Tone acknowledgement/reload, voice/handwriting handoff safety, slow status
  probes, native-worker cancellation, and multilingual GPU readback using Mesa software rendering
  and installed CJK fonts.
  The job also verifies data-folder actions against a private FileManager1 fixture, builds the four
  Linux release binaries, packages `.tar.gz`/`.deb`, checks extracted contents and uploads packages
  plus checksums as 14-day CI artifacts. None of these checks installs a package on the host.
- **macOS and Windows** compile-check GPU-enabled desktop code and test targets on native runners;
  these checks do not claim native input-method or graphical runtime coverage.

The native checks use temporary settings and a deterministic local model fixture. They do not
download Llama, contact a real model, capture the desktop, or change the desktop's input method.
They check actual model-request temperatures, rejected/failed configuration writes, draft/context
boundaries on model reload, and persistence across host restart. Single-instance tests cover
concurrent launches, crash recovery and legacy running panels without opening desktop windows.
Candidate tests also cover exact consecutive commits, standalone delivery acknowledgements,
confirmation gates, and retained drafts after rejection or a lost acknowledgement.
Reproduce them on Linux after installing the packages listed in `.github/workflows/ci.yml`:

```bash
bash scripts/test-linux-ci.sh ibus
LIBGL_ALWAYS_SOFTWARE=1 bash scripts/test-linux-ci.sh ui
```

CI does not yet gate on the known strict-Clippy/default-feature issues above, package Android,
publish releases, or claim real-model quality/latency coverage. A committed workflow is not proof
of a passing run: GitHub Actions must be enabled, and reading private-repository results requires
a GitHub connection authorized for this repository.

### 0.5.0 UI refinements

- Allocate handwriting title, status, guidance, canvas and footer using full text line heights.
  Narrow guidance can wrap onto two rows without moving the canvas when status changes; Undo,
  Clear and candidate chips share a non-overlapping row. Ink is clipped to the canvas after resize.
- Drag from the header, margins and other non-interactive space in the main panel, settings or
  floating icon. Functional hit targets (including their touch padding) take priority; the seed
  text row remains editable, and dragging across a button cannot click it on release. Grab/grabbing
  cursors distinguish window movement from text editing, handwriting and scale/scroll controls.
- Fit the native panel height to the visible input mode and candidates on startup, tab fold/unfold,
  zoom and content changes. Width-driven control scaling avoids height/scale feedback; the panel
  stays top-anchored during resize and no longer has a fixed 1360px content-width cap.
- Deduplicate native resize requests and keep delayed programmatic size acknowledgements out of
  the zoom base, including a quick shrink-and-restore before the first acknowledgement arrives.
  Content fitting respects the monitor height limit; settings keep their independent window.
- Switch minimum/maximum size constraints when entering the 92×92 floating icon, then restore the
  main panel constraints and its previous decoration style when expanding again.
- Render visible Unicode glyphs on demand, with per-character system-font fallback instead of
  replacing Chinese/Japanese candidates with question marks. Repeated frames reuse glyphs and
  GPU coordinates; the atlas has a fixed memory budget and evicts stale glyphs when full.
- Use full-width CJK geometry consistently for candidate labels, wrapping, cursor measurements
  and scrolling. Collapsed CJK candidates share available width so alternate readings stay visible.
- Center keyboard, candidate-chip, and toolbar labels within their button surfaces at every text scale.
- Share one settings layout between standalone and embedded views, with measured option widths and font-aware row heights.
- Keep matching settings expanded during search, show an empty-result hint, and clip scrolling text and click targets to the content viewport.
- Draw tooltips and commit feedback above the full panel scene; delay hover hints until a 600 ms dwell, hide them during input, and keep them inside the window without idle redraw polling.

#### Font compatibility and visual verification

The panel keeps the chosen UI font and falls back per missing character. On Linux, Fontconfig
selects a language-appropriate CJK face (including the face index inside `.ttc` collections);
known Noto/WenQuanYi paths cover systems without Fontconfig. macOS and Windows have native CJK
fallback paths as well; Windows font paths respect `WINDIR`. Only Linux has been exercised on
hardware for this change. A CJK font must be installed (this machine uses Noto Sans CJK SC/JP).
After installing fonts, restart the panel. Unsupported glyphs use a distinct missing-glyph marker,
not literal `?` candidate text; the window title also reports missing coverage. Color-emoji and
complex-script shaping are not provided by this single-glyph renderer.

The opt-in visual test renders synthetic Chinese, English and Japanese candidates using the actual
panel shaders, font atlas and GPU readback, including 2.5× scaling, collapsed/expanded layouts,
bounded-cache eviction and repeated-frame reuse. Handwriting scenes additionally cover 420–2500px
widths at medium/large text sizes. It does not capture the desktop or read typed text:

```bash
cargo test --all-features --offline --bin panel \
  multilingual_candidates_render_through_the_gpu_without_question_mark_fallback -- --ignored --nocapture
# Optional: set SUZAKU_GLYPH_QA_DIR to an output directory to retain synthetic BMP images.
cargo test --all-features --offline -- --test-threads=1
# In an isolated test IBus session with English selected:
target/release/linux_ime_probe --complete hel   # Tab + Space (editable), then Enter; verifies exact text
```

Some existing FFI tests share global theme state and can interfere under parallel test execution;
the serial regression command avoids that interference. This does not remove the repository-wide
strict-Clippy and default-feature limitations described above.

The isolated UI suite also checks voice/handwriting handoff acknowledgements, source retention on
failure and late replies, and Linux's consume-once transcript path with an injected clock. It uses
only synthetic text, with live voice capabilities disabled; no microphone is opened. This validates
panel/bridge integration, not production Linux speech recognition. Text-output tests use a private
Unix socket, and a cross-platform source guard prevents macOS speech logging from being reintroduced.

The Linux native-window regression uses an isolated Xvfb display and temporary settings. It tests
keyboard/voice/handwriting folding, zoom and restore through real window-size events. It also checks
mouse/touch drag routing, control priority, cancellation, releases over other controls, compact
background clicks, and actual X11 window movement. It does not capture the desktop, switch its input
method or inject global input events. The fixed English example goes from 900×487 expanded to
900×165 folded and back without zoom-base drift:

```bash
suzaku_fit_qa=$(mktemp -d /tmp/suzaku-fit-qa.XXXXXX)
env -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$suzaku_fit_qa/config" \
  SUZAKU_IME_CONFIG="$suzaku_fit_qa/ime.json" SUZAKU_PANEL_NATIVE_QA=1 \
  xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
  cargo test --all-features --offline --bin panel \
  native_window_fits_content_through_fold_zoom_and_restore -- --ignored --nocapture
```

Geometry tests also cover empty/non-empty English/CJK candidates, three input modes and scaled
viewports; GPU readback uses the fitted height rather than a fixed large canvas. Only Linux native
window behavior has been exercised on this machine.

The keyboard regression sends X11 key events only to windows it creates on an isolated Xvfb
display. It checks actual focus acquisition after clicking the input field, text/digits/spaces on
all three input tabs, IME preedit/commit routing, focus return, and self-target commit protection:

```bash
suzaku_key_qa=$(mktemp -d /tmp/suzaku-keyboard-qa.XXXXXX)
env -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$suzaku_key_qa/config" \
  SUZAKU_IME_CONFIG="$suzaku_key_qa/ime.json" SUZAKU_PANEL_NATIVE_QA=1 \
  SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
  xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
  cargo test --all-features --offline --bin panel \
  native_keyboard_editing_and_focus_return -- --ignored --nocapture
```

### 0.4.9 Highlights

- Added a native Linux IBus engine host, user-service installer, runtime diagnostics, and a
  private panel-to-host commit channel.
- Added a Suzaku system tray with show/hide, settings, position reset, and quit actions, plus
  per-session single-instance activation.
- Refined the Ubuntu / GNOME Wayland panel with non-focusing input, monitor-bound dragging,
  close-to-tray behavior, compact outer spacing, and correctly positioned settings windows.
- Reduced desktop input latency through idle-aware redraw scheduling, reusable GPU buffers,
  cached system-font atlases, displayed-scene hit testing, and width-aware candidate wrapping.
- Improved the Android IME with clipboard privacy controls, batched native render snapshots,
  interruption-safe animations, and frame-aligned handwriting updates.

### 0.4.8 Highlights

- Scaled-aware text rendering pipeline for clearer zoomed desktop UI.
- Performance-safe font atlas rebuild behavior on window scale changes.
- Updated font atlas and sampling defaults to improve readability at larger scales.
- Tunable handwritten-stroke sampling and smoothing via environment variables, enabling faster on-device touch calibration for 0.4.8.
- Added theme presets for desktop panel visuals.
  - Supported values in `panel-settings.toml` (`theme_preset`):
    - `daylight`
    - `device_dark`
    - `solarized`
    - `high_contrast`
  - Default config paths:
    - Linux: `~/.config/suzaku-panel/panel-settings.toml` (or `$XDG_CONFIG_HOME/suzaku-panel/panel-settings.toml`)
    - macOS: `~/Library/Application Support/SuzakuPanel/panel-settings.toml`
    - Windows: `%APPDATA%/SuzakuPanel/panel-settings.toml`

The repository currently contains three layers that evolve together:

- a shared Rust IME core
- platform-specific system IME host adapters
- debug and companion UIs for desktop and Android

The codebase is no longer a single-file prototype. The current shape is:

- Shared engine: [src/ime.rs](src/ime.rs)
- Host session contract: [src/ime_host.rs](src/ime_host.rs)
- Platform adapters: [src/platform](src/platform)
- Desktop GPU companion: [src/bin/panel.rs](src/bin/panel.rs)
- Android IME app: [android](android)

## Architecture

### 1. Shared IME Core

The shared IME core owns:

- seed text normalization
- candidate generation
- candidate selection
- commit flow
- language-plugin dispatch

Key files:

- [src/ime/core_engine.rs](src/ime/core_engine.rs)
- [src/languages/mod.rs](src/languages/mod.rs)
- [src/languages/english.rs](src/languages/english.rs)
- [src/languages/llm.rs](src/languages/llm.rs)
- [src/languages/llama.rs](src/languages/llama.rs)

### 2. System IME Host Layer

The host layer is where platform-native IME lifecycles connect to the shared session.

Core contract:

- [src/ime_host.rs](src/ime_host.rs)

Adapter and lifecycle contract:

- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)

Dispatch and capability map:

- [src/platform/mod.rs](src/platform/mod.rs)
- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)
- [src/platform/ime_host_dispatch.rs](src/platform/ime_host_dispatch.rs)
- [src/platform/ime_host_runtime.rs](src/platform/ime_host_runtime.rs)
- [src/platform/panel_companion_dispatch.rs](src/platform/panel_companion_dispatch.rs)

Current platform direction:

- macOS: InputMethodKit host path is the most complete
- Windows: TSF skeleton is present
- Android: InputMethodService app path is active
- Linux: native IBus engine host is available; Fcitx remains a registration scaffold

### 3. Companion UIs

The project uses two kinds of companion UI:

- desktop GPU debug companion
- native or app-hosted candidate/input surfaces per platform

Desktop GPU scene builders live in:

- [src/ime/gpu](src/ime/gpu)

Companion style lives in:

- [src/platform/companion_style.rs](src/platform/companion_style.rs)

## Repository Layout

### Core Rust

- [src/lib.rs](src/lib.rs)
- [src/ime.rs](src/ime.rs)
- [src/ime_host.rs](src/ime_host.rs)
- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)
- [src/platform/ime_host_runtime.rs](src/platform/ime_host_runtime.rs)
- [src/panel_support.rs](src/panel_support.rs)

### Desktop Panel

- [src/bin/panel.rs](src/bin/panel.rs)
- [src/bin/panel](src/bin/panel)

### macOS

- [src/platform/macos_ime.rs](src/platform/macos_ime.rs)
- [src/macos/ime_host_bridge.m](src/macos/ime_host_bridge.m)
- [src/macos/speech_bridge.m](src/macos/speech_bridge.m)
- [src/macos/text_output_bridge.m](src/macos/text_output_bridge.m)

### Android

- [src/platform/android_ime.rs](src/platform/android_ime.rs)
- [src/platform/android_jni_bridge.rs](src/platform/android_jni_bridge.rs)
- [android/app/src/main/java/dev/suzaku/android/ime](android/app/src/main/java/dev/suzaku/android/ime)

### Concept Notes

The original system notes are still part of the repository and now serve as background design documents:

- [src/00-Map.md](src/00-Map.md)
- [src/01-Signal.md](src/01-Signal.md)
- [src/02-Control.md](src/02-Control.md)
- [src/03-Resilience.md](src/03-Resilience.md)
- [src/04-Coordination.md](src/04-Coordination.md)
- [src/05-Expression.md](src/05-Expression.md)
- [src/06-IME-XR-Tablet.md](src/06-IME-XR-Tablet.md)

## Main Commands

### Rust checks

```bash
cargo check --features gpu
./scripts/test-gpu-smoke.sh
cargo test --features gpu
cargo build --features gpu
cargo build --release --features gpu
cargo run --release --features gpu --bin suzaku-map -- --help
cargo ime-host
```

### Desktop GPU companion

```bash
cargo run --features gpu --bin panel
```

Or use one-command launcher:

```bash
./scripts/panel-gpu.sh
./scripts/panel-gpu.sh release
./scripts/panel-gpu.sh build-release
```

#### Current local priority: Ubuntu / Wayland

The desktop GPU companion is the primary development path on the current workstation. Its event
loop sleeps while the UI is idle, wakes at frame cadence only for active voice/feedback/text-scroll
animation, reuses streaming GPU vertex buffers, and uses the last displayed scene for pointer hit
testing. Installed platform fonts are rasterized into a cached GPU atlas, with the built-in bitmap
font retained only as a startup fallback. On GNOME Wayland with XWayland available, the main panel
automatically uses a non-focusing X11 override-redirect window. It opens at the bottom center,
continues to accept pointer/touch input, and leaves IBus focus in the target application while
candidates are clicked. The top-center grip provides manual drag handling for this unmanaged window,
keeps the panel inside the active monitor, and preserves its expanded position. The default 100%
layout uses a compact outer gutter, a dedicated close-to-tray button, and readable two-column
candidates with an unpaired candidate spanning the final row. A native StatusNotifierItem with a
Suzaku red-and-gold icon remains available after the panel is hidden: click it to show or hide the
panel, or use its menu to activate/release the input method, open settings, reset the window
position, and quit the process. Activating Suzaku hides the large companion and remembers the
previous IBus engine; native candidates appear only while composing in the target app. Releasing
or normally quitting restores the previous engine, unless the user has already switched to another
input method. A failed restore keeps the tray open with a retryable error. The
settings window is placed above the main panel and clamped to the active monitor. Tray-state updates
run off the UI thread. Launching the panel again signals the existing process to show its window
rather than creating duplicate windows or tray icons. Set
`SUZAKU_LINUX_PANEL_BACKEND=wayland` to force the native Wayland window path.
Cross-platform adapters remain in the tree, but local desktop behavior is validated first.

### GPU smoke test profile

The GPU smoke suite includes deterministic loopback HTTP fixtures. No downloaded model is required.

```bash
# Local validation (default)
cargo test-gpu-smoke
./scripts/test-gpu-smoke.sh
```

### Integration validation (local-loopback llama bridge required)

```bash
cargo test-gpu-smoke-llm
./scripts/test-gpu-smoke.sh llm-bridge
```

### Common build note (important)

The root crate is feature-gated for GPU components.
Builds without `--features gpu` may fail with errors about missing `winit` or `ime::gpu` items.
For desktop companion/scene/debug work, prefer:

```bash
cargo build --features gpu
cargo run --features gpu --bin panel
cargo run --release --features gpu --bin suzaku-map -- --help
```

Shortcuts from Cargo aliases:

```bash
cargo panel-macos
# GPU-focused tests:
cargo test-gpu                 # raw GPU test run (no --nocapture)
cargo test-gpu-smoke           # local smoke run (bridge test skipped unless enabled)
cargo test-gpu-smoke-llm       # includes a deterministic local HTTP fixture, not a live model
```

### macOS panel bundle

```bash
cargo panel-app-macos
cargo open-panel-app-macos
cargo install-panel-app-macos
cargo install-open-panel-app-macos
```

### macOS IME bundle

```bash
cargo ime-app-macos
cargo open-ime-app-macos
cargo install-ime-app-macos
cargo install-open-ime-app-macos
cargo enable-ime-app-macos
cargo install-enable-ime-app-macos
```

### Android

Environment check:

```bash
cargo android-doctor
```

Linux registration helper:

```bash
cargo linux-register install
cargo linux-register status
cargo linux-register uninstall
cargo linux-register verify
cargo linux-register diag
```

Rust-side host bootstrap:

```bash
cargo ime-host
cargo android-ime-host
```

Native helper actions (Rust-native runner):

```bash
eval "$(cargo android-env)"
cargo android-build-native
cd android && ./gradlew assembleDebug
cargo android-install-debug
cargo android-enable-ime
```

### Handwriting tuning (desktop panel)

The desktop panel supports runtime tuning for touch/mouse handwriting capture through environment variables (set before launch):

- `SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS` (default `8`) – minimum touch sample interval, milliseconds.
- `SUZAKU_HANDWRITING_MOUSE_SAMPLE_MS` (default `4`) – minimum mouse sample interval, milliseconds.
- `SUZAKU_HANDWRITING_EDGE_PADDING` (default `4.0`) – in-canvas inset for clamped stroke coordinates.
- `SUZAKU_HANDWRITING_MIN_POINT_DISTANCE` (default `1.2`) – base minimum point distance.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP` (default `3.2`) – base interpolation segment size.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA` (default `0.22`) – base smoothing amount.
- `SUZAKU_HANDWRITING_SPEED_REFERENCE` (default `16.0`) – speed reference for adaptive profile.
- `SUZAKU_HANDWRITING_SPEED_RATIO_MIN` (default `0.55`) – minimum speed ratio.
- `SUZAKU_HANDWRITING_SPEED_RATIO_MAX` (default `2.0`) – maximum speed ratio.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP_MIN` (default `2.0`) – lower clamp for interpolation step.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP_MAX` (default `6.8`) – upper clamp for interpolation step.
- `SUZAKU_HANDWRITING_MIN_DISTANCE_MIN` (default `0.78`) – lower clamp for min distance.
- `SUZAKU_HANDWRITING_MIN_DISTANCE_MAX` (default `2.0`) – upper clamp for min distance.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN` (default `0.14`) – lower smoothing clamp.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX` (default `0.34`) – upper smoothing clamp.
- `SUZAKU_HANDWRITING_TOUCH_SMOOTH_FACTOR` (default `1.05`) – extra touch smoothing multiplier.
- `SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE` (default `1.18`) – touch distance scale.
- `SUZAKU_HANDWRITING_TOUCH_INTERPOLATION_SCALE` (default `0.9`) – touch interpolation scale.
- `SUZAKU_HANDWRITING_TOUCH_SAMPLE_DISTANCE_SCALE` (default `0.85`) – touch-specific distance scale.

Example:

```bash
SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS=6 SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE=1.2 cargo run --features gpu --bin panel
```

Recommended default entry (no script dependency):

```bash
eval "$(cargo android-env)"
cargo android-build-native
cargo android-install-debug
cargo android-enable-ime
cargo linux-register install
```

APK output (generated locally after an Android build, not tracked in the repository):

`android/app/build/outputs/apk/debug/app-debug.apk`

## Current Platform Status

### macOS

Most complete desktop path today.

Available:

- GPU panel companion
- app bundle packaging
- IMK host bootstrap
- marked text / commit roundtrip skeleton
- native candidate companion window path
- voice bridge

### Windows

Scaffolded:

- platform capability map
- IME host skeleton
- voice backend skeleton

### Android

Active app-host path.

Available:

- InputMethodService app
- shared HostImeSession integration
- adaptive letter, number, and symbol keyboard layouts
- one-shot Shift, Caps Lock, editor actions, and Unicode-safe repeating backspace
- context-aware email/URI shortcuts, dedicated numeric/phone/date-time pads, and action labels
- system keyboard switching plus candidate-boundary spaces, punctuation, and Enter actions
- app-provided Android completions and stale-composition cancellation on cursor movement
- transient clipboard paste drawer with sensitive-preview protection and no saved history
- persistent Android controls for auto-capitalization, a dedicated number row, and haptics
- a setup/settings hub for enabling Suzaku, choosing it, sharing preferences, and diagnostics
- keyboard, voice, and handwrite drawers
- secure password-field input with candidates, voice, and handwriting disabled
- compact-bubble-first activation flow
- in-panel candidate strip
- batched native render snapshots, reused keyboard/candidate views, interruption-safe panel
  animation, and frame-aligned handwriting redraws for a smoother input loop

### Linux

Current workstation-first path: the Rust GPU companion and native IBus host on Ubuntu / GNOME
Wayland.

- event-driven desktop redraw with idle suspension and bounded animation wakeups
- reusable GPU vertex buffers and displayed-scene pointer hit testing
- platform-font GPU atlas rendering with a deterministic bitmap fallback
- settings-window synchronization without redraw loops or repeated idle config writes
- bottom-centered, pointer-active, non-focusing XWayland panel mode on GNOME Wayland, with an
  explicit native-Wayland override, visible drag grip, and monitor-bound position clamping
- explicit click-to-type focus for the panel input field, independent of keyboard/voice/handwriting
  tabs; regular text, digits, repeat keys and single spaces take priority over panel commands
- OS IME preedit/commit input for Chinese, Japanese and accented text; preedit stays a display-only
  preview until committed. The event lifecycle follows the [winit IME contract](https://docs.rs/winit/0.30.12/winit/event/enum.Ime.html).
- Enter / Esc finishes panel editing and restores its borrowed X11 focus if the user has not
  already switched apps; show, drag and candidate clicks never request editing focus. On native
  Wayland and other platforms, focus activation/restoration remains subject to the OS/window manager.
  Before sending a candidate, focus the target app; Linux rejects commits into the panel itself.
- StatusNotifierItem system tray with a built-in Suzaku icon, show/hide activation, position reset,
  direct settings access, close-to-tray behavior, and an explicit quit action
- per-session single-instance control that reopens the existing panel on a repeated launch;
  a process-lifetime OS lock serializes startup and releases on crash. The empty `panel.lock`
  file is intentionally retained after clean shutdown so contenders always lock the same file.
- Ubuntu / Arch / SteamOS capability profiles
- Linux voice backend and probe path
- native IBus `Factory`/`Engine` host backed by the shared Rust candidate engine
- engine-owned IBus drafts shown in the candidate area's auxiliary line by default, with six
  visible candidate rows, mixed word/sentence ranking, and bounded annotated previews;
  arrow/Tab navigation, paging, 1–6 editable selection in all three languages, Alt+digits
  literal draft input, candidate clicks, Space continuation, and explicit commit
- tray-controlled IBus activation/restoration with verified switches, bounded off-thread host I/O,
  on-menu-open status refresh, and no global keyboard hook or input polling
- native roundtrip probe coverage for input-triggered preedit/candidates, final commit, and dismissal
- local user-only panel-to-IBus commit channel at `$XDG_RUNTIME_DIR/suzaku-ime/host.sock`
- live native-to-panel composition subscription: preedit, exact candidate labels/text, selection,
  language and asynchronous AI updates come from the native host rather than a second LLM request
- revision-checked panel candidate commits and word/on-screen completion edits; stale clicks, old
  host instances and private contexts are rejected without retrying or falling back to a new target
- truthful runtime registration status:
  - bootstrap reads `SUZAKU_LINUX_IME_FRAMEWORK=fcitx` to switch to Fcitx checks
  - a component marker alone does not imply marked-text or commit readiness
  - IBus static and dynamically registered engines are probed separately from daemon and host state
  - quick local override for staging: `SUZAKU_LINUX_IME_REGISTERED=1`
  - lifecycle simulation overrides for host checks:
    - `SUZAKU_LINUX_IME_DAEMON_READY=1|0`
    - `SUZAKU_LINUX_IME_RUNTIME_VISIBLE=1|0`
    - `SUZAKU_LINUX_IME_ACTIVE=1|0`
    - `SUZAKU_LINUX_IME_HOST_READY=1|0`
    - `SUZAKU_LINUX_IME_MARKED_TEXT=1|0`
    - `SUZAKU_LINUX_IME_COMMIT=1|0`
    - `SUZAKU_LINUX_IME_NATIVE_CANDIDATE_WINDOW=1|0`

Build and install the native host without root:

```bash
# Normally install the build headers once through your distribution:
# sudo apt install libibus-1.0-dev
cargo build --release --features linux-ibus --bin linux_ime_host --bin suzaku_tool
SUZAKU_LINUX_IME_HOST_BIN="$PWD/target/release/linux_ime_host" \
  target/release/suzaku_tool linux-register install
target/release/suzaku_tool linux-register verify
```

The installer copies the host to `~/.local/libexec/suzaku/linux_ime_host`, enables
`~/.config/systemd/user/suzaku-ibus.service`, dynamically registers
`dev.suzaku.linux.ime` with the running IBus daemon, and safely appends Suzaku to GNOME's input
source switcher when that setting is available. During a host upgrade it records and restores
the active IBus engine, so restarting the service does not leave the desktop on an empty engine.
After installation, launching `panel` ensures the registered user service is ready without
changing the selected input method. Right-click its tray icon:

- **激活 Suzaku 输入法** first starts a stopped host, then selects the native engine and hides the large panel. Focus a text field
  and type to summon the cursor-anchored candidate window; on the non-focusing Linux backend the
  companion also reappears as a collapsed candidate view. No held shortcut is required.
- **释放并恢复：…** restores the input method used just before activation. The menu shows that
  actual engine, not a guessed English/default layout. Repeated activation preserves it.
- **退出 Suzaku** (or the full quit shortcut) restores the observed previous input method, stops
  the managed host service, then exits. Restore/stop failures leave the UI available for retry.
  Hiding the panel or closing it to the tray leaves input working. A manually selected different
  engine is never overwritten on release. The release action alone keeps the host ready for reuse.

This control currently targets Linux / IBus. It does not install global shortcuts, change the
configured input-source list, or automatically take over at startup. If Suzaku was already selected
outside this tray session, switch back through the system input-source menu; there is no known
previous engine to restore. Private/custom host endpoints remain owned by their original launcher,
not by the desktop service controller. Forced process termination is not a normal tray quit and cannot run
the session's restore step.

Native companion synchronization uses a persistent `W` subscription on the existing user-only
host socket. Frames carry a host-instance UUID, focus-context ID and monotonic revision. The host
pushes on input, selection, prediction, commit/cancel and focus/privacy changes; the panel queues
only the newest frame and does not run its own predictor while mirroring. Slow readers are
disconnected using nonblocking writes, and a restarted host supplies a fresh snapshot on reconnect.
The settings/status (`S`) channel remains text-free.

Clicking a mirrored candidate submits that exact native candidate through a revision-checked `A`
action; word chips and the on-screen keyboard update native preedit through the same channel.
There is no automatic retry or fallback commit when focus/content/selection changes. A candidate
press is cancelled if its displayed revision changes before release. Directly clicking the seed
input returns to the independent panel draft and the normal click-to-type behavior.

Password/PIN and application-marked private contexts send empty frames, never text or candidates;
surrounding text and committed history are never included. Privacy detection depends on the client
reporting its IBus content purpose/hints. Automatically summoned views hide after completion or
focus loss; an explicitly shown panel stays open but clears native text. Manually hiding a native
view suppresses automatic reopening for that focus context. This transport is implemented for
Linux/IBus; other platform hosts still need their own native subscription adapters.

The real native sync test runs its own D-Bus and IBus daemon, creates synthetic input contexts,
and uses a deterministic loopback model fixture (not the user's model/settings or desktop):

```bash
bash scripts/test-linux-ci.sh ibus
```

This checks full candidate/selection equality, late subscriptions, panel commits and preedit edits,
stale actions across input contexts and host restarts, asynchronous AI updates, English/Chinese/
Japanese, privacy, Escape and focus-out. Direct engine peers also check reordered focus notifications,
late key/navigation/click events, field-context isolation and primary-click selection on both pages.
The runner builds and locates the activation test executable, verifies the test exists, and exercises
the tray controller's real activation/input/release path in all three languages, including cleanup
restoration. Special/invalid configuration reloads must leave both native keys and the host responsive.
The Xvfb keyboard regression additionally exercises the panel's native-view routing, independent draft
preservation, and cancellation of outdated presses.

You can also use `Super+Space` to select **Suzaku**, type a Latin seed, use arrows or Tab to
move through candidates, `Page Up` / `Page Down` to change pages, and `1`–`6` to adopt a candidate
without committing. Space and punctuation continue the draft; Enter or a left click submits it.
Without an active draft, ordinary digits go directly to the application. Alt+digits explicitly
enters literal numbers into the draft. Escape cancels the current draft.

The 0.6.3 IBus presentation places each superscript shortcut inside the candidate's text
hit target and leaves the separate shortcut label empty. Clicking the visible ordinal therefore
uses the same correct row index as clicking its text, including on later pages; commit payloads
and number-key adoption are unchanged. See the [ordinal compatibility fix](docs/bug-audit-ordinal-click-2026-09-23.md).

Unconfirmed text appears in the candidate area/companion panel, not in the application's inline
preedit buffer. This prevents clients such as Qt and Chrome from committing cached draft text
when focus changes. The 0.6.3 candidate-area preview keeps a suffix within 48 display cells
(wide characters count as two, zero-width marks as zero), capped at 160 Unicode code points.
This limits unwrapped native labels, not the draft or committed text; see the
[native popup audit](docs/bug-audit-native-popup-2026-09-23.md). Advanced users may start the host with
`SUZAKU_IBUS_INLINE_PREEDIT=1` to restore inline preedit, but that also restores the risk of
client-initiated commits on focus changes. See the
[cross-application fix audit](docs/bug-audit-cross-app-fixes-2026-09-23.md) for tested boundaries.

`linux-register uninstall`
removes Suzaku from the GNOME switcher, disables the user service, and removes the installed host
and component metadata.

Useful commands:

- `cargo linux-register install`
- `cargo linux-register status`
- `cargo linux-register uninstall`
- `cargo linux-register verify`
- `cargo linux-register diag`
- `cargo run --features linux-ibus --bin linux_ime_probe -- ni`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --ipc "panel commit probe"`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --llm nihao`
  (requires an enabled, running local model; waits for an AI candidate and selects it)
- `cargo run --features linux-ibus --bin linux_ime_probe -- --private nihao`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --password nihao`
- `bash scripts/test-linux-ci.sh ibus` (private activation/input/release and native regression suite)
  (opt-in live-desktop check: activates, types in an isolated input context, then restores)

Framework selection is shared with bootstrap:

- `SUZAKU_LINUX_IME_FRAMEWORK=fcitx` to register and check Fcitx layout
- default remains IBus when not set

Fcitx currently retains marker/status support; a native Fcitx engine service is still pending.

## Optional bottom panel layout (unreleased)

Display settings persist `panel_layout_mode=auto|follow-caret|bottom-dock`; missing or invalid
values default to Auto. Existing explicit values remain manual overrides. Backup validation
accepts all three preferences, never the machine-specific `detected_panel_layout` runtime field.
`PanelChromeState::effective_panel_layout()` resolves Auto before rendering, positioning and
native input lifecycle decisions; settings windows inherit the main window's cached detection.
The automatic option displays the detected choice even when another layout is selected.

`src/platform/panel_layout.rs` performs a bounded, local startup probe: Android prefers bottom;
desktop/unknown operating systems prefer caret following. Linux recognizes exact mobile/gaming
session tokens, then valid `/etc/machine-info` `CHASSIS`, DMI chassis type, and DeviceTree chassis
type in that order. A configured desktop/laptop classification overrides contradictory hardware
fallbacks. Unknown metadata, missing/unreadable files, convertibles and detachable devices without
a known posture remain caret-following. Mere touchscreen presence, display size, distribution,
or `XDG_CURRENT_DESKTOP=gamescope` from a nested game do not trigger the bottom layout.
The conservative chassis categories follow the [systemd hostnamed mapping](https://github.com/systemd/systemd/blob/main/src/hostname/hostnamed.c).
No device event stream, device identifier, external command or network request is used. Detection
is not live posture/hot-plug monitoring and does not qualify native mobile window integration.

The GPU layout is separate from Linux caret tracking: `PanelLayoutMode` changes scene ordering
and touch target sizes, while `src/bin/panel/bottom_layout.rs` owns monitor bounds, size/position
requests and restoring the previous floating width. Bottom dock is an overlay, not a reserved
desktop work area or a native Wayland input-method surface.

An expanded native keyboard may remain after submission only for the already-woken public
host/context. Context changes, privacy, focus loss, disconnect and local editing clear that
retention; empty fields alone do not authorize it. Asynchronous window growth waits for a
size acknowledgement before drawing the enlarged page, with the existing bounded resize
deadline preventing a permanent wait on a clamping window manager.

`bash scripts/test-linux-apps.sh bottom-layout` exercises actual settings clicks and restart
persistence, all native candidate slots, pagination, screen-key continuation after commit,
orb restoration, folding and switching back to caret following on an owned 1024×768 Xvfb
display. Run both the default and `SUZAKU_GTK3_QA_SYNC_MODE=1` transports; screenshots/logs are
retained with `SUZAKU_APP_QA_KEEP=1`. `candidates` remains the separate full caret-follow gate.
`auto-layout` starts without saved settings in a private Phosh-labelled session and uses the
production detector. Its 36 workflows include actual manual override, restarting with that
override, restoring Auto, restarting again, and the shared bottom-layout input checks.
Each layout now includes three private offline-pack byte-budget workflows: Unicode
single-entry overflow, aggregate-frame overflow and JSON escaping. They install with the
adjacent `suzaku_tool` before host startup, then verify real drawing, number adoption,
Backspace undo and exact card-click submission. Current follow/bottom/auto counts are
26/33/36 per transport; the previous counts below are historical validation records.
Both transports of all three suites are included in the Linux CI workflow. This does not qualify
physical touch, GNOME Shell/native Wayland positioning, or install the changed binaries into a
desktop session.

Local validation on 2026-10-03 passed the all-features regression, final panel regression,
strict Clippy, ShellCheck, 17 private native UI checks, and both IBus transports for all
30 bottom-layout plus 23 follow-caret workflows. First failures exposed hidden-window
fixture clicks, overlapping collapsed controls, an over-constrained vertical hit tolerance,
and candidate rendering before the resize acknowledgement; the fixes retain the original
timeouts and assertions. The follow-restore fixture now moves to a genuinely different
vertical caret anchor rather than requiring movement from an already edge-clamped position.

The automatic-layout follow-up passed 1,079 non-ignored all-features tests, strict Clippy,
ShellCheck, the 17 opt-in native UI checks, both transports of the 33 Auto and 23 caret-follow
workflows, and the 30 manual-bottom workflows with the default transport. The desktop fixture
resolved this machine to `follow-caret` via `linux-dmi-non-mobile`; the private Phosh fixture
resolved to `bottom-dock` while preserving explicit overrides across process restarts.
An extra `cargo check --locked --no-default-features --all-targets` did not pass: unchanged
core icon/platform modules still import GPU types and `winit` without feature guards. This
existing feature-boundary issue is not repaired by the layout work; the validated build uses
`--all-features`. Its first failure is retained separately from the passing regression logs.

## Refactor Policy

This repository is intentionally being kept in a refactor-friendly early state.

Current codebase rules that the project is trying to hold:

- keep modules small enough to reason about
- separate shared IME logic from platform lifecycles
- separate system-host code from debug companion UI
- let README describe the current codebase, not the historical prototype

## Current Refactor Snapshot

This repository was recently cleaned up in two important ways:

- the GPU scene builder no longer carries the old unused `*.inc` and `*_macro` fragments
- the top-level README now describes the current architecture instead of the original single-file prototype story

That makes the current structure a better base for continued platform work, especially Android and system-host integration.
