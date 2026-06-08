# Account Setup Flow

Do **not** run `setup_account.py` bare inside LibrAgent. The old terminal wizard asks for multiple prompts and can time out.

## Setup Steps

1. Determine the email address.
2. For preset domains, see [server-profiles.md](server-profiles.md).
3. For custom/self-hosted domains, collect all four values in chat: IMAP host/port and SMTP host/port.
4. Collect the password through a **single hidden shell prompt**, store temporarily in the persistent shell, then run `setup_account.py` in a separate visible command.

## PowerShell Pattern (LibrAgent)

Run `runInPersistentPowerShell` with:

- `requireUserInput=true`
- `inputType=password`
- `inputPrompt` like `이메일 비밀번호 또는 앱 비밀번호를 입력하세요:`
- `timeout=90`

**Two-step flow:**

1. Hidden prompt: set temporary password env var
2. Visible setup: run `setup_account.py ... --password-env ...`
3. Cleanup: remove the temporary env var

Hidden prompt:

```powershell
$env:LIBRAGENT_EMAIL_PASSWORD = Read-Host
```

Preset domain setup:

```powershell
python "<skill-base-dir>/scripts/setup_account.py" `
  --email "user@example.com" `
  --use-preset-servers `
  --password-env LIBRAGENT_EMAIL_PASSWORD
```

Reset after auth failure:

```powershell
python "<skill-base-dir>/scripts/setup_account.py" `
  --reset `
  --email "user@example.com" `
  --use-preset-servers `
  --password-env LIBRAGENT_EMAIL_PASSWORD
```

Custom domain setup:

```powershell
python "<skill-base-dir>/scripts/setup_account.py" `
  --email "user@example.com" `
  --imap-host "imap.example.com" `
  --imap-port 993 `
  --smtp-host "smtp.example.com" `
  --smtp-port 587 `
  --password-env LIBRAGENT_EMAIL_PASSWORD
```

Cleanup:

```powershell
Remove-Item Env:LIBRAGENT_EMAIL_PASSWORD -ErrorAction SilentlyContinue
```

On non-PowerShell shells: same two-step pattern, or use `--password-stdin` if the runtime can pipe hidden input securely.

Replace `<skill-base-dir>` with this skill's absolute Base Directory.
