# Security Policy

## Reporting Vulnerabilities

**Do not open public issues for security vulnerabilities.**

Email security reports to: [win@dedaluslabs.ai](mailto:win@dedaluslabs.ai)

Include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge your report within 48 hours and provide a detailed response within 7 days.

## Supported Versions

| Version | Supported                |
| ------- | ------------------------ |
| main    | Active development       |
| < 1.0   | Pre-release, best-effort |

## Security Considerations

microvm handles:

- **Virtualization.framework**: Direct Apple API access requiring entitlements
- **Host resources**: Memory and CPU allocation capped at 75% of host
- **Disk images**: Rootfs and block devices passed through to the guest
- **virtiofs shares**: Host directories exposed to the guest
- **Snapshots**: Machine state persisted to disk (opaque Apple format)

### Threat Model

microvm runs VMs with the same privilege as the calling user. It does not
provide isolation beyond what Apple's Virtualization.framework enforces.
Do not run untrusted guest code without understanding the VZ sandbox boundary.

## Disclosure Policy

We follow coordinated disclosure:

1. Reporter submits vulnerability privately
2. We acknowledge within 48 hours
3. We investigate and develop fix
4. We release fix and credit reporter (unless anonymity requested)
5. Public disclosure after 90 days or when fix is deployed
