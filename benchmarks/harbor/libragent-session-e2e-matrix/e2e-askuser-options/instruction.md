Ask me three questions in ONE `ui__askUser` call, with exactly these ids:

1. `city` — "Which city should we visit?", with options Seoul, Tokyo, Busan.
2. `toppings` — "Pick pizza toppings", with options Cheese, Onion, Pepperoni
   and `multiSelect: true`.
3. `note` — "Any extra note?" (free text, no options).

When the answers come back, write the tool result verbatim to
`ask_result.json`. Do not reformat, reorder, or paraphrase it.
