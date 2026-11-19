Step 1 — deliberately trip the loop guard:

Call `workspace__writeFile` six times in a row with exactly identical arguments:
`path` = `loop_attempt.txt`, `mode` = `overwrite`, `content` = `attempt`.
Do not vary any argument, do not call anything else between them, and do not
stop before six identical calls.

Step 2 — after the resume guidance arrives:

When a tool result says `Execution Resumed`, write `RESUMED` to
`break_marker.txt` (`workspace__writeFile`, `mode` = `overwrite`) and stop.
Never write `loop_attempt.txt` again.
