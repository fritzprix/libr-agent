# Calendar Setup Flow

Do **not** ask for `client_secret` in chat. Collect it with one `workspace__runInPersistentShell` call that has `requireUserInput=true`, `inputType=password`, and `storeAs: "LIBRAGENT_GOOGLE_CLIENT_SECRET"`. LibrAgent asks for the value, stores it for this session, and gives it to the command as `$LIBRAGENT_GOOGLE_CLIENT_SECRET`. The secret never appears in the command, the chat, or the result. No cleanup step is needed.

## First-time connect

Collect `client_id` in chat (it is not secret), then run:

```json
{
  "command": "python \"<skill-base-dir>/scripts/setup.py\" --client-id \"YOUR_CLIENT_ID.apps.googleusercontent.com\" --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET --oauth",
  "requireUserInput": true,
  "inputType": "password",
  "inputPrompt": "Google OAuth client secret을 입력하세요:",
  "storeAs": "LIBRAGENT_GOOGLE_CLIENT_SECRET"
}
```

## Re-auth (tokens expired / revoked)

If `validate_config.py` returns `action: oauth` and client credentials are already saved, no secret prompt is needed:

```bash
python "<skill-base-dir>/scripts/setup.py" --oauth
```

## Full reset

Use the same call shape as first-time connect with this command:

```text
python "<skill-base-dir>/scripts/setup.py" --reset --client-id "YOUR_CLIENT_ID.apps.googleusercontent.com" --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET --oauth
```

If `LIBRAGENT_GOOGLE_CLIENT_SECRET` is already stored in this session and is correct, you can run the command without `requireUserInput`.

Replace `<skill-base-dir>` with this skill's absolute Base Directory.
