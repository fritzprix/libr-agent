# CalDAV Setup (Future)

**Not available in the MVP.** This document reserves the integration pattern for a later release.

## Planned providers

- Nextcloud / ownCloud
- Apple iCloud (app-specific password)
- Fastmail and other standards-compliant CalDAV servers

## Planned config shape

```json
{
  "provider": "caldav",
  "google": null,
  "caldav": {
    "url": "https://nextcloud.example.com/remote.php/dav/calendars/user/personal",
    "username": "user",
    "password": "..."
  }
}
```

Authentication will follow the `email-integration` hidden-password pattern (no OAuth).

Until CalDAV ships, Google Calendar users should use [google-setup.md](google-setup.md).
