# Account Setup Flow

Do **not** run `setup_account.py` bare inside LibrAgent. The old terminal wizard asks for multiple prompts and can time out.

## Setup Steps

1. Determine the email address.
2. For preset domains, see [server-profiles.md](server-profiles.md).
3. For custom/self-hosted domains, collect all four values in chat: IMAP host/port and SMTP host/port.
4. Run `setup_account.py` in **one** `workspace__runInPersistentShell` call with `requireUserInput=true`, `inputType=password`, and `storeAs: "LIBRAGENT_EMAIL_PASSWORD"`. LibrAgent asks for the password, stores it for this session, and gives it to the command as `$LIBRAGENT_EMAIL_PASSWORD`. The password never appears in the command, the chat, or the result.

## Tool call

Preset domain setup:

```json
{
  "command": "python \"<skill-base-dir>/scripts/setup_account.py\" --email \"user@example.com\" --use-preset-servers --password-env LIBRAGENT_EMAIL_PASSWORD",
  "requireUserInput": true,
  "inputType": "password",
  "inputPrompt": "이메일 비밀번호 또는 앱 비밀번호를 입력하세요:",
  "storeAs": "LIBRAGENT_EMAIL_PASSWORD",
  "timeout": 90
}
```

Use the same call shape with these commands:

- Reset after auth failure (ask for the password again, because the stored one was wrong):

  ```text
  python "<skill-base-dir>/scripts/setup_account.py" --reset --email "user@example.com" --use-preset-servers --password-env LIBRAGENT_EMAIL_PASSWORD
  ```

- Custom domain setup:

  ```text
  python "<skill-base-dir>/scripts/setup_account.py" --email "user@example.com" --imap-host "imap.example.com" --imap-port 993 --smtp-host "smtp.example.com" --smtp-port 587 --password-env LIBRAGENT_EMAIL_PASSWORD
  ```

If `LIBRAGENT_EMAIL_PASSWORD` is already stored in this session and is correct, you can run the command without `requireUserInput`. No cleanup step is needed.

Exit code `3` means the variable is empty or unset. Ask for it with `requireUserInput=true` and `storeAs`, then run setup again.

Replace `<skill-base-dir>` with this skill's absolute Base Directory.
