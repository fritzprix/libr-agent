Step 1 — call a tool with a string-typed boolean:

Call `planning__getCurrentState` with `include_checked` set to the JSON
string `"true"` — pass it literally as a string, not as a boolean.

Step 2 — record that the call worked:

Once the tool returns successfully, write exactly `COERCION_OK` to
`bool_ok.txt`.

Step 3 — ask a question with a reserved-sounding key:

Call `ui__askUser` with a single question whose `id` is exactly `options`
and whose text asks to pick a color. Do not rename the id.

Step 4 — record the answer:

Write the answer you receive verbatim to `ask_answer.txt`.
