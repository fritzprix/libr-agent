This task verifies the bundled-skill `storeAs` secret collection contract
(2565 S-1, S-2, S-5, S-6 and 2685 E-3) in the persistent shell without making
any network calls. An automated harness answers each password prompt with a
distinct synthetic value. Never echo, cat, or print any secret value itself —
only its SHA-256 hex digest.

Run these four `workspace__runInPersistentShell` calls in order, each with
`requireUserInput: true` and `inputType: "password"`:

1. `storeAs: "TELEGRAM_API_HASH"`, `inputPrompt: "Enter TELEGRAM_API_HASH"`, command:
   `printf '%s' "$TELEGRAM_API_HASH" | sha256sum | awk '{print $1}' > telegram.sha256`

2. `storeAs: "X_PASSWORD"`, `inputPrompt: "Enter X_PASSWORD"`, command:
   `printf '%s' "$X_PASSWORD" | sha256sum | awk '{print $1}' > x.sha256`

3. `storeAs: "LIBRAGENT_EMAIL_PASSWORD"`, `inputPrompt: "Enter LIBRAGENT_EMAIL_PASSWORD"`, command:
   `printf '%s' "$LIBRAGENT_EMAIL_PASSWORD" | sha256sum | awk '{print $1}' > email.sha256`

4. `storeAs: "LIBRAGENT_GOOGLE_CLIENT_SECRET"`, `inputPrompt: "Enter LIBRAGENT_GOOGLE_CLIENT_SECRET"`, command:
   `printf '%s' "$LIBRAGENT_GOOGLE_CLIENT_SECRET" | sha256sum | awk '{print $1}' > calendar.sha256`

Then stop. Never print any of those environment variable values anywhere.
