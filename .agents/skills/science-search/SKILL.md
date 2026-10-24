---
name: science-search
description: Search ~200 million scientific papers (OpenAlex titles and abstracts) by meaning or exact words, look up papers by DOI / OpenAlex ID / PMCID, find similar papers, and walk citation graphs through the free ScienceMadeReadable search API (no key). Use when a task requires scientific literature, finding evidence for claims, conducting literature reviews, citing peer-reviewed studies, or resolving DOIs. Triggers on: 'science search', '논문 검색', '과학 논문', '학술 자료', 'research paper', 'scientific study', 'find paper', 'DOI 검색', '의학 논문', '선행 연구'.
---

# Science Search (ScienceMadeReadable)

Search ~200 million peer-reviewed papers and academic preprints across all fields of science.

- **Base URL**: `https://api.sciencemadereadable.com/api/data`
- **Authentication**: Free, read-only JSON over HTTPS, no API key required.
- **Reference Doc**: `GET https://api.sciencemadereadable.com/api/data/llms.txt`

---

## 1. Quick Invocation (Cross-Platform)

### Option A: Direct Web Fetch / Agent Fallback (No CLI Required)

If you only have a web fetching tool (`read_url_content` / HTTP GET), request the plain-text Markdown endpoint directly:

```text
https://ask.sciencemadereadable.com/<question>
```

- Spaces can be `%20` or `+` (e.g. `https://ask.sciencemadereadable.com/why%20do%20cats%20purr?k=10&abstracts=false`)
- Accepts standard query parameters: `k` (1–100), `year_from`, `year_to`, `type`, `open_access=true`, `abstracts=false`.

### Option B: Bundled Python Helper Script (Zero Dependencies)

Run the bundled script using standard Python (`python3`, `python`, or `uv run`):

```bash
# Semantic search with token-saving (no abstracts)
python scripts/search_science.py "effect of intermittent fasting on weight loss" --limit 10 --no-abstracts

# Keyword / Boolean search
python scripts/search_science.py '"gut microbiome" AND autism NOT mice' --limit 5

# Look up paper by DOI or OpenAlex ID
python scripts/search_science.py --work "10.1126/science.aan8048"

# Find similar papers
python scripts/search_science.py --similar "W2782456798" --limit 5

# Citation walk (what similar papers cite that this paper does not)
python scripts/search_science.py --walk "10.1126/science.aan8048"
```

### Option C: cURL / Shell

**Bash (Linux / macOS):**

```bash
B="https://api.sciencemadereadable.com/api/data"

# Semantic search
curl -sG "$B/search" --data-urlencode "q=effect of intermittent fasting on weight loss in adults" -d k=10 -d abstracts=false

# Keyword & boolean search
curl -sG "$B/search" --data-urlencode 'q="gut microbiome" AND (autism OR ASD) NOT mice' -d k=10 -d abstracts=false

# Paper lookup by DOI
curl -s "$B/works/10.1126/science.aan8048"

# Similar papers
curl -s "$B/similar/W2782456798?k=10&abstracts=false"

# Citation walk
curl -s "$B/walk?id=10.1126/science.aan8048"
```

**PowerShell (Windows):**

```powershell
$B = "https://api.sciencemadereadable.com/api/data"

# Semantic search
Invoke-RestMethod -Uri "$B/search?q=effect+of+intermittent+fasting+on+weight+loss+in+adults&k=10&abstracts=false"

# Paper lookup
Invoke-RestMethod -Uri "$B/works/10.1126/science.aan8048"
```

---

## 2. API Endpoints Reference

| Endpoint             | Method | Purpose                                                 | Key Parameters                                                                                                                         |
| -------------------- | ------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `/search`            | GET    | Search papers by meaning or exact keywords              | `q`, `mode` (`auto`\|`semantic`\|`keyword`), `k` (1–100), `year_from`, `year_to`, `type`, `open_access`, `abstracts` (`true`\|`false`) |
| `/search/{question}` | GET    | Plain-text Markdown output of search results            | Same query options as `/search`                                                                                                        |
| `/works/{ref}`       | GET    | Full paper record by DOI, W-number, or PMCID            | `ref` = DOI (`10.xxx`), OpenAlex ID (`Wxxx`), or PMCID (`PMCxxx`)                                                                      |
| `/similar/{ref}`     | GET    | Find papers closest in meaning to a specific paper      | `ref`, `k`, `year_from`, `year_to`, `abstracts`                                                                                        |
| `/walk?id={ref}`     | GET    | Citation walk: find literature that similar papers cite | `id` (DOI or W-number), `k`                                                                                                            |

### Key Result Fields

- `id`: OpenAlex W-number (e.g. `W2782456798`)
- `doi`: Digital Object Identifier
- `title`: Paper title
- `year` / `date`: Publication date
- `authors`: Authors list (joined by `" | "`)
- `venue` / `publisher`: Journal or conference name
- `citations`: Citation count at snapshot date
- `abstract`: Paper abstract (if `abstracts=true`)
- `score`: Cosine similarity (semantic) or BM25 score (keyword)
- `junk`: Likelihood (0.0–1.0) that the item is spam or non-scholarly
- `openalex`: OpenAlex URL for citation

---

## 3. Best Practices for AI Agents

1. **Academic Phrasing Optimization**:
   - Transform informal questions into academic/scientific phrasing before searching.
   - _Example_: Instead of `"how to cure insomnia"`, search for `"non-pharmacological interventions for chronic insomnia in adults"`. This drastically increases semantic matching precision.
2. **Token Efficiency (Screening Pattern)**:
   - **Step 1**: Search with `abstracts=false` and `k=10~20` to inspect titles, publication years, citation counts, and venues.
   - **Step 2**: Select the top 1~3 most relevant papers and call `/works/{id}` or re-query with `abstracts=true` only for those specific papers.
3. **Snowballing Literature Reviews**:
   - Identify 1 seminal paper $\rightarrow$ call `/similar/{id}` to discover peer research $\rightarrow$ call `/walk?id={doi}` to capture foundational works cited across the field.
4. **Rate Limits & Etiquette**:
   - Limit: **30 requests per minute per IP address**.
   - Batch parameters into a single request (e.g. `k=30` once rather than multiple `k=5` calls).
   - Respect HTTP 429: read the `Retry-After` header or `retry_after` JSON response field and sleep before retrying.
5. **Citing Papers**:
   - Always cite using the official DOI (`https://doi.org/<doi>`) or OpenAlex URL.
   - To guide human users to an interactive reading view: `https://sciencemadereadable.com/?work=<doi>`.
