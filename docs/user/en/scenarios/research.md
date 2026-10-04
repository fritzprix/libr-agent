---
title: Scenario - Deep Research & Report Generation
---

# Scenario: Deep Research & Report Generation

When you provide a research topic, the agent searches the web, gathers reference data, synthesizes insights, and writes structured reports. Use this workflow for market studies, technical reviews, and competitor analysis.

---

## 🎯 Objectives

- Investigate complex topics systematically to produce comprehensive reports.
- Compare and analyze multiple sources to reach reliable conclusions.
- Generate structured documents with authoritative citations and data tables.

---

## 📋 Step-by-Step Guide

### Step 1: Specify Research Topics

State your research objective clearly to the agent.

**Basic request:**

```
Investigate current trends in the AI agent platform market.
```

**Detailed request:**

```
Investigate the commercial readiness of autonomous driving technology in 2026.
Include industry leader status, technical bottlenecks, and expected timelines.
```

### Step 2: Define Research Scope (Optional)

Configure boundaries, research depth, and output formatting.

**Define time and language boundaries:**

```
Investigate this topic with the following constraints:
- Use sources published within the past 12 months.
- Include both English and Korean publications.
- Cite at least 5 independent authoritative sources.
- Present comparative data in tables.
```

**Specify depth:**

```
Investigate this topic at three distinct levels of depth:
1. Executive Overview (1 page).
2. Detailed Technical Breakdown.
3. Industry Implications and Future Outlook.
```

**Specify report structure:**

```
Format the findings into this structure:
- Executive Summary
- Current State Analysis
- Competitor Comparison Table
- Future Outlook
- Reference List
```

### Step 3: Inspect Intermediate Findings

You can inspect the agent's progress during execution.

**Check interim findings:**

```
Summarize your research progress so far.
Indicate which sources provide the highest quality evidence.
```

**Direct further exploration:**

```
Search for news published in the past two weeks.
Find specific numerical statistics and market adoption percentages.
```

### Step 4: Review and Refine Results

Review the generated report and request targeted adjustments.

**Request specific revisions:**

```
The report is clear. Refine these specific sections:
1. Expand the regional vendor analysis for East Asia.
2. Update key statistics with the newest available quarterly data.
3. Add actionable recommendations to the conclusion.
```

**Save report to disk:**

```
Save the finalized report as a Markdown document named:
'2026-08-08_AIAgentMarketAnalysis.md'.
```

---

## 💡 Example Prompt Collection

| Purpose | Prompt |
| --- | --- |
| Broad research | `"Conduct an in-depth investigation on {topic}."` |
| Market sizing | `"Research market size, CAGR growth rate, and key vendors for {market}."` |
| Competitor analysis | `"Compare products from {Company A} and {Company B} in a structured matrix."` |
| Tech evaluation | `"Investigate technology trends in {tech} over the past 6 months."` |
| News synthesis | `"Summarize recent developments on {topic} with source links."` |
| Formal report | `"Compile the research findings into an executive report with key takeaways."` |
| Reference audit | `"List all references and URLs used in this report in alphabetical order."` |
| Comparative table | `"Create a feature comparison matrix between {Item A} and {Item B}."` |

---

## 📊 Report Structure Template

The agent compiles reports using this standard template:

```markdown
# [Report Title]

## 📋 Executive Summary

A single concise paragraph summarizing core conclusions.

## 📊 Current State Analysis

- Key statistics and numerical findings.
- Market size, adoption metrics, and industry velocity.

## 🔍 In-Depth Breakdown

- Thematic evaluation across sub-topics.
- Comparative matrices and technical insights.

## 🏢 Major Organizations and Vendors

- Overview.
- Current product positioning.
- Strategic roadmap.

## 🔮 Future Outlook

- Emerging opportunities and potential risks.
- Strategic implications for stakeholders.

## 📚 References

1. [Source Title](URL) - Summary of cited evidence.
2. ...
```

---

## ⚙️ Configuration Tips

### Create a Research Specialist Assistant

1. Open `/assistants` and select **New Assistant**.
2. System prompt:
   ```
   You are an expert research analyst.
   Provide factual, evidence-backed conclusions and always cite primary sources.
   Mark unverified claims clearly as estimates or hypotheses.
   ```
3. Enable web search and browser sidecar tools.

### Maximize Source Credibility

- Instruct the agent to cite primary sources rather than secondary aggregators.
- Request explicit confirmation of publication dates to prevent stale data.
- Instruct the agent to flag contradictions between conflicting sources.

---

## 🔗 Related Documentation

- [Web Browsing Scenario](./web-browsing.md) — Web search and content extraction workflows
- [Playbooks Guide](../guides/playbooks.md) — Save research workflows as reusable playbooks

---

## 🚀 Next Steps

| Project | Description | Difficulty |
| --- | --- | --- |
| Weekly Industry Digest | Generate recurring weekly research reports | ⭐⭐ |
| Competitor Intelligence | Track changes in rival offerings continuously | ⭐⭐⭐ |
| Academic Paper Summary | Extract methodology and results from academic papers | ⭐⭐⭐ |
| Product Evaluation Matrix | Build comprehensive comparative feature matrices | ⭐⭐ |
| Research Playbook | Turn your favorite research steps into a playbook | ⭐⭐ |
| Automated Report Delivery | Save finished research documents to cloud directories | ⭐⭐ |
