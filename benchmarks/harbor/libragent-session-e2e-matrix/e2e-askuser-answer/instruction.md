Call `ui__askUser` exactly once, with a single free-text question (no options):
`id` = `color`, `question` = `Favorite color?`.

The harness answers the dialog automatically. Read the answer text from the
tool result and write it verbatim to `answer.txt` in the workspace root using
`workspace__writeFile`.

Never invent or modify the answer. If the tool result is an error, report it
verbatim in your final summary and do not write the file.
