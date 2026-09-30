# Diagrams & math (chat / UI markdown)

Use these in assistant replies, `ui__presentInteractive` (`format: "markdown"` / `auto`), or `ui__reportResult`. No skill script required.

## Mermaid

Supported for visualization asks: `flowchart`, `sequenceDiagram`, `classDiagram`, `erDiagram`, `stateDiagram-v2`, `mindmap`, `gantt` (as needed).

Mindmap:

````markdown
```mermaid
mindmap
  root((Visualize))
    Diagrams
      Mermaid
      Mindmap
    Math
      LaTeX
    Charts
      CSV
      Excel
```
````

Flowchart:

````markdown
```mermaid
flowchart TD
  A[Request] --> B{Tabular data?}
  B -->|yes| C[charts.md + visualize.py]
  B -->|no| D[Mermaid or LaTeX in chat]
```
````

Sequence:

````markdown
```mermaid
sequenceDiagram
  participant U as User
  participant A as Agent
  U->>A: Explain the pipeline
  A->>U: Mermaid sequence in the reply
```
````

## LaTeX (KaTeX)

Inline: `$L = \frac{1}{n}\sum_i (y_i - \hat{y}_i)^2$`.

Display:

````markdown
$$
\nabla_\theta \mathcal{L}(\theta) = 0
$$
````

## UI tools

- Prefer markdown mode for Mermaid/LaTeX — it already renders.
- Do not switch to `format: "html"` only to show a diagram.
