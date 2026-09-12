# Security policy

Suzaku is an Alpha input method. The latest preview receives fixes; older versions have no
maintained security-backport schedule or response-time guarantee.

## Report privately

Do not publish tokens, private input, exploit payloads or sensitive logs in issues or pull requests.
If the repository's Security tab offers **Report a vulnerability**, use that private channel.
If unavailable, open a minimal issue asking the maintainer to arrange a private reporting channel,
without disclosing the vulnerability or private data. Wait for that channel before sending details.
A dedicated contact address and private-reporting availability are not yet guaranteed.

A private report should include version, platform, impact and a minimal synthetic reproduction.
Never include a live credential; revoke exposed credentials with the provider first.

## Boundaries

Input privacy, target isolation, bounded local IPC, model consent and credential handling are in
scope. Model accuracy and a gateway's independent forwarding/retention policies are not guaranteed.
See [privacy boundaries](docs/privacy.md).

Keep a fallback input method. Backups are unencrypted. Checksums detect corruption, not publisher
identity. History/dependency scans are useful checks, not proof of freedom from secrets or flaws.
