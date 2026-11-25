This task verifies startup recovery when the app restarts while a `ui__askUser` prompt is waiting (2565 P-8 & 2672 A-4).

Call `ui__askUser` with one text question:
- id `"confirmation"`, label `"Confirm restart test"`, type `"text"`

Wait for the response. The automated harness will restart the isolated instance while the prompt is waiting.
