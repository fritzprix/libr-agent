# Error Handling

| Error | Cause | Action |
| --- | --- | --- |
| Config not found | First run | Run [setup-flow.md](setup-flow.md) |
| `auth_invalid` / token refresh failed | Revoked or expired refresh token | Re-run `setup.py --oauth` |
| `dependency_missing` | Python libs not installed | `pip install google-auth google-auth-oauthlib google-api-python-client` |
| OAuth `access_denied` | Consent screen / test user | See [google-setup.md](google-setup.md) |
| `invalid_grant` | Bad client secret or revoked app | `--reset` setup with correct Desktop client |
| 404 on event | Wrong `event_id` | `list_events` or `week_summary` to find ID |
| Timezone confusion | Ambiguous local time | Confirm user timezone; use RFC 3339 offsets |

Always give the **next concrete command**, not only the error text.
