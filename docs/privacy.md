# Privacy boundaries

An active input method necessarily processes keystrokes, preedit and selected candidates.
These are implementation boundaries, not a security certification.

## Local data

Drafts, candidate lists, translations, handwriting/voice drafts and short committed context remain
in memory. Suzaku does not persist input history. Backups include recognized settings and key-variable
names, not key values, drafts, model weights or transcripts. Backups are unencrypted and may reveal
provider addresses and preferences; review before sharing. See [data management](linux-packaging-data.md).

The native companion channel contains active preedit/candidates, not surrounding application text
or accumulated committed history. Password/PIN and declared numeric/decimal/phone fields bypass
composition and panel injection. Private hints suppress model requests and companion snapshots.
Applications must report purposes/hints correctly; switch input methods when unsure about a field.

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
