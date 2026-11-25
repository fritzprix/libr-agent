Step 1 — ask me a question:

Call `ui__askUser` with a single question asking which city to visit.

Step 2 — handle the outcome:

If the question comes back cancelled, that is the expected outcome — record
it by writing exactly `CANCELLED_OK` to `cancel.txt`, state in one short
line that the dialog was cancelled, and finish. Do not retry the question,
do not treat the cancellation as a failure, and do not ask again.
