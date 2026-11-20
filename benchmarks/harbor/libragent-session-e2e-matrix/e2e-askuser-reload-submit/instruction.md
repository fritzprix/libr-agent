This task verifies that a waiting `ui__askUser` prompt survives a session reload/re-poll and delivers its JSON answer so the turn continues (2565 P-5 & X-5).

Step 1 — Call `ui__askUser` with two questions:
- id `"choice"`, label `"Pick option"`, type `"select"`, options `["alpha", "beta"]`
- id `"note"`, label `"Add note"`, type `"text"`

Step 2 — After receiving the user's answer, write the received `"choice"` and `"note"` values into `/app/reload_answer.txt` in the format `choice=<choice>;note=<note>`, then stop.
