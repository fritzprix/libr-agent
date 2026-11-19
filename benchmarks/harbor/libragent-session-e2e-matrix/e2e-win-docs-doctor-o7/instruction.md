This task verifies 2565 O-7 on Windows: the `docs` skill doctor runs cleanly without an NTFS junction loop and reports `Doctor: OK`.

Step 1 — Load the `docs` skill (`getSkillContent` for `"docs"`) to locate its `scripts/convert.py` path.

Step 2 — Run the `docs` skill doctor (`python <skill-base-dir>/scripts/convert.py --doctor`) using a PowerShell workspace tool, capturing stdout and stderr into `o7_doctor_output.txt` in the workspace.

Step 3 — Stop once `o7_doctor_output.txt` is written.
