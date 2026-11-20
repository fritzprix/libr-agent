This task verifies 2565 O-7 on Windows: the `docs` skill's documented Windows doctor script runs cleanly without an NTFS junction loop and reports `Doctor: OK`.

Step 1 — Load the `docs` skill (`getSkillContent` for `"docs"`) to locate its `<skill-base-dir>`.

Step 2 — Using a PowerShell workspace tool (`workspace__runPowerShell` or `workspace__runInPersistentPowerShell`), run the documented Windows doctor command:
`powershell -NoProfile -ExecutionPolicy Bypass -File "<skill-base-dir>/scripts/doctor.ps1"`

Step 3 — Once the command completes and prints `Doctor: OK`, stop.
