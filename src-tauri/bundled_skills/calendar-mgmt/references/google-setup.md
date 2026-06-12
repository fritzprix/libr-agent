# Google Cloud Setup (Calendar API)

One-time setup per user. Same model as Telegram API credentials — **the user brings their own OAuth client**.

## Steps

1. Open [Google Cloud Console](https://console.cloud.google.com/).
2. Create or select a project.
3. **APIs & Services → Library** → enable **Google Calendar API**.
4. **APIs & Services → OAuth consent screen**
   - User type: **External** (personal Gmail) or **Internal** (Workspace)
   - Add scope: `https://www.googleapis.com/auth/calendar`
   - Add your Google account as a test user while in "Testing" mode
5. **APIs & Services → Credentials → Create credentials → OAuth client ID**
   - Application type: **Desktop app**
   - Copy **Client ID** (safe to paste in chat)
   - Copy **Client secret** (never paste in chat — use hidden prompt only)

## LibrAgent handoff

After step 5:

1. Tell the user the Desktop client is required (not Web application).
2. Collect **client_id** in chat.
3. Run hidden password prompt for **client_secret**.
4. Execute `setup.py --client-id ... --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET --oauth`.

## Common issues

| Issue | Fix |
| --- | --- |
| `access_denied` on consent | Add user as test user on OAuth consent screen |
| `redirect_uri_mismatch` | Client must be **Desktop app**, not Web |
| Calendar API disabled | Enable Google Calendar API in the library |
