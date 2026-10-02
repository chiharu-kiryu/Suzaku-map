# Privacy boundaries

An active input method necessarily processes keystrokes, preedit and selected candidates.
These are implementation boundaries, not a security certification.

## Local data

Drafts, candidate lists, translations, handwriting/voice drafts and short committed context remain
in memory. Suzaku does not persist input history. Backups include recognized settings and key-variable
names, not key values, drafts, model weights or transcripts. Backups are unencrypted and may reveal
provider addresses and preferences; review before sharing. See [data management](linux-packaging-data.md).

Optional offline packs are explicitly imported vocabulary, not learned input history. Their manifests
and text persist unencrypted in the user's `suzaku/lexicons/registry.json` data directory, outside
settings backups. Pack tools do not collect drafts or access model services; example collections are
authored by the project. Third-party content/author/license claims are unverified: review packs and
never include private records in packages you distribute. Pack validation is not signature verification.
Once a pack's suggestion is adopted into an ordinary draft, existing opt-in provider rules apply to
that draft exactly as for manually typed text; importing vocabulary does not grant cloud consent.

The 0.7.3 IBus frequency cache is separate from settings and dictionaries. It retains
at most 2,048 salted fingerprints of language/query/candidate tuples and decaying counts, rather
than draft text. Hashing is not encryption or an anonymity guarantee. Confirmed
preferences can affect existing suggestions across ordinary fields during the host's lifetime;
the cache never generates text or sends its contents to a model. Private fields neither read
nor update it. Pending adoptions are discarded on cancellation, editing away the chosen prefix,
focus/language/provider/privacy boundaries, or cache clear. A number/Shift+Enter/Space adoption
is not learned until its text survives to a successful engine commit. Native delivery still uses
IBus's normal commit signal, not an application-level receipt.

No cache file, input history, backup entry or background persistence worker is created. The tray's
**Clear candidate preferences (memory only)** action discards both confirmed and pending learning
without changing a live selection; the next draft rebuild uses defaults. Restarting the host also
forgets it. This is process-lifetime personalization, not a persistent user dictionary; unhinted
masked fields have the same limitations described below.

The native companion channel contains active preedit/candidates, not surrounding application text
or accumulated committed history. Password/PIN and declared numeric/decimal/phone fields bypass
composition and panel injection. Private hints suppress model requests and companion snapshots.
Applications must report purposes/hints correctly; switch input methods when unsure about a field.
For example, local testing of Zenity 4.0.1 `--entry --hide-text` found that it masks text visually
but reports an ordinary unhinted field to IBus, unlike its `--password` dialog. Visual masking
alone does not guarantee private handling; disabling the model also does not hide local companion
drafts. See the [GTK application audit](bug-audit-gtk-apps-2026-09-23.md).
The tested Chrome 153 X11/GTK path and Qt 5.15.13/6.4.2 IBus plugins also do not convey numeric
field purposes. 0.6.2 passes digits directly while the draft is empty, without
creating candidates; this does not mark the whole field private. Letters, explicit Alt+digits
and panel actions can still use ordinary-field routing when purposes are missing. Standard
password keyboard paths passed isolated tests, but this does not qualify arbitrary masked
controls or companion injection. See the [compatibility fixes](bug-audit-cross-app-fixes-2026-09-23.md).

0.6.1 also rejects the independent panel's direct text sends into PRIVATE
fields, while keeping native local conversion available. Changing the active field's private/bypass
policy invalidates its old target identity, including delayed direct sends; a brief round trip back
to public input does not revive them. Ordinary hint updates and stale events from another engine
do not change the current target. See the [content-type audit](bug-audit-content-type-2026-09-23.md).
0.6.1 also invalidates old targets after a successful input-language change, even
when intermediate language snapshots are coalesced away. Same-language aliases, provider-only
reloads and failed changes keep the target. See the [language-target audit](bug-audit-language-target-2026-09-23.md).

Native focus/language/privacy changes clear session context and pending keyboard replay. Uncertain
sends are never retried automatically. Source/recovery text can remain visible until explicitly
handled, its native context changes or the process ends.

## Optional providers

Suggestions require opt-in. Requests include language, raw draft, local conversion and up to 160
characters of committed context within the current focus session. Explicit translation uses the
source draft and language choices. Providers may log, retain or forward data under their own policies,
including through a local gateway.

Local discovery checks fixed loopback endpoints without downloading models, reading model files or
scanning the network. Cloud use requires an explicit HTTPS endpoint/model and consent. Credentials
come from named process environment variables; never put values in URLs, settings or CLI arguments.
Changing provider identity through the CLI or restoring a backup revokes cloud consent. Disabling
requests cannot retract data already sent. See [provider configuration](model-providers.md).

## Reports

Use synthetic text. Review screenshots, paths, usernames, logs and settings before sharing.
Automated fixtures use synthetic inputs and private sessions. A model probe can send synthetic
samples to the configured provider and incur charges. Follow [SECURITY.md](../SECURITY.md) for
security reports, without putting passwords, tokens or real drafts in public issues.
