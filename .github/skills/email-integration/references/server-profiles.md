# Mail Server Profiles

Reference for `setup_account.py` auto-detection and manual troubleshooting.
When a domain matches, these settings are applied as defaults (user can override).

---

## Gmail (`gmail.com`, `googlemail.com`)

| Setting | Value |
| --- | --- |
| IMAP host | `imap.gmail.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.gmail.com` |
| SMTP port | `587` (STARTTLS) |

**⚠️ App Password required when 2-Step Verification is on.**

1. Go to [Google Account Security](https://myaccount.google.com/security)
2. Under "How you sign in to Google" → **2-Step Verification** → scroll to bottom → **App passwords**
3. Select "Mail" + "Windows Computer" (or "Other") → Generate
4. Use the 16-character code as your password

**If IMAP is not enabled:**
Gmail Settings → See all settings → Forwarding and POP/IMAP → Enable IMAP

---

## Outlook / Hotmail (`outlook.com`, `hotmail.com`, `live.com`)

| Setting | Value |
| --- | --- |
| IMAP host | `outlook.office365.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.office365.com` |
| SMTP port | `587` (STARTTLS) |

**Enable IMAP:**
[Outlook Settings](https://outlook.live.com/mail/options/mail/popimap) → POP and IMAP → IMAP enabled

**App Password (if 2FA is on):**
[Microsoft Security](https://account.microsoft.com/security) → Advanced security options → App passwords

---

## Microsoft 365 / Work Outlook (`*.onmicrosoft.com`, corporate domains)

| Setting | Value |
| --- | --- |
| IMAP host | `outlook.office365.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.office365.com` |
| SMTP port | `587` (STARTTLS) |

**Note:** Corporate M365 tenants may block Basic Auth (username/password).
If login fails, the admin needs to enable SMTP AUTH per-mailbox:
`Exchange Admin Center → Mailboxes → [user] → Mail flow settings → Authenticated SMTP`

---

## Naver Mail (`naver.com`)

| Setting | Value |
| --- | --- |
| IMAP host | `imap.naver.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.naver.com` |
| SMTP port | `587` (STARTTLS) |

**Enable IMAP first (required):**
Naver Mail → 환경설정 → POP3/IMAP 설정 → IMAP/SMTP 설정 → **사용함** 선택

Use your **Naver ID password** (not a separate app password).

---

## Daum / Kakao Mail (`daum.net`, `kakao.com`)

| Setting | Value |
| --- | --- |
| IMAP host | `imap.daum.net` / `imap.kakao.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.daum.net` / `smtp.kakao.com` |
| SMTP port | `465` (SSL direct) |

Use your **Kakao account password**.

---

## Yahoo Mail (`yahoo.com`)

| Setting | Value |
| --- | --- |
| IMAP host | `imap.mail.yahoo.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.mail.yahoo.com` |
| SMTP port | `587` (STARTTLS) |

**App Password required:**
[Yahoo Account Security](https://login.yahoo.com/account/security) → Generate app password → Mail

---

## iCloud Mail (`icloud.com`, `me.com`, `mac.com`)

| Setting | Value |
| --- | --- |
| IMAP host | `imap.mail.me.com` |
| IMAP port | `993` (SSL) |
| SMTP host | `smtp.mail.me.com` |
| SMTP port | `587` (STARTTLS) |

**App-specific password required:**
[Apple ID](https://appleid.apple.com) → Sign-In and Security → App-Specific Passwords

---

## Custom / Self-Hosted

No preset. `setup_account.py` will prompt for all fields manually.

Common patterns:

| Provider type | Typical IMAP port | Typical SMTP port |
| --- | --- | --- |
| SSL (recommended) | `993` | `465` |
| STARTTLS | `143` | `587` |
| Legacy (no TLS) | `143` | `25` (avoid) |

If connecting to port `465` for SMTP, the `smtp_use_tls` flag will be set to `false`
in the config (direct SSL instead of STARTTLS).

---

## Port Quick Reference

| Port | Protocol | Usage |
| --- | --- | --- |
| 993 | IMAP over SSL | Standard secure IMAP |
| 143 | IMAP + STARTTLS | Alternative (less common) |
| 587 | SMTP + STARTTLS | Standard submission port |
| 465 | SMTP over SSL | Legacy SSL submission |
| 25 | SMTP plain | Server-to-server only, avoid |
