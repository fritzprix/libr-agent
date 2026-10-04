---
title: Scenario - Web Browsing & Information Gathering
---

# Scenario: Web Browsing & Information Gathering

When you provide a search query or target URLs, the agent collects, analyzes, and summarizes real-time information from the web. Use this workflow for news monitoring, product comparisons, and technology trend research.

---

## 🎯 Objectives

- Collect and summarize real-time web content automatically.
- Compare and synthesize information across multiple websites.
- Automate repetitive web investigation workflows.

---

## 📋 Step-by-Step Guide

### Step 1: Provide Search Queries or URLs

Give the agent a research topic or specific website URLs.

**Search query prompt:**

```
Investigate market trends for AI agent platforms in 2026.
```

**Direct URL prompt:**

```
Summarize the latest articles from this page:
https://techcrunch.com/category/artificial-intelligence/
```

**Compare multiple URLs:**

```
Compare and analyze these three product websites:
- https://example.com/product-a
- https://example.com/product-b
- https://example.com/product-c
```

### Step 2: Instruct Browsing Actions

The agent uses browser automation tools to visit web pages and extract content.

**Basic browsing:**

```
Visit "https://news.ycombinator.com/" and summarize top 5 recent AI discussions.
```

**Extract specific data:**

```
Extract the top 10 trending Python projects from "https://github.com/trending".
```

**In-page documentation search:**

```
Search "https://docs.python.org/3/" for "async" documentation and summarize key points.
```

### Step 3: Review Structured Results

The agent compiles the collected web findings into a structured report.

**Typical result format:**

```markdown
## 📰 Market Trends: AI Agent Platforms

### Key Developments

1. **[Platform A Secures Seed Funding]**
   - Source: TechCrunch (2026-08-05)
   - Summary: Raised $5M in Series A funding...
   - Link: https://techcrunch.com/...

2. **[Platform B Launches Enterprise Suite]**
   - Source: The Verge (2026-08-03)
   - Summary: Released an Enterprise AI agent platform with zero-trust security...
   - Link: https://theverge.com/...

### Market Insights

- Leading Players: Company A, Company B, Company C.
- Market Size: Estimated at $XX Billion in 2026.
- Key Trends:
  - Rapid enterprise adoption.
  - Open-source platform competition.
  - Multi-modal agent execution.
```

### Step 4: Request Deeper Analysis

Refine the collected information with follow-up prompts.

**Comparative analysis:**

```
Compare Platform A and Platform B in a table. Include pricing, features, and supported models.
```

**Add recent updates:**

```
Search for additional news published in the past 24 hours on this topic.
```

**Validate sources:**

```
Evaluate the credibility of these sources. Indicate which citations are primary industry sources.
```

**Save to disk:**

```
Save this research report to "./research/ai-platform-trends-2026.md".
```

---

## 💡 Example Prompt Collection

| Purpose | Prompt |
| --- | --- |
| News search | `"Summarize top 10 news articles from the past week regarding {topic}."` |
| Web page summary | `"Summarize the key takeaways from https://example.com."` |
| Product comparison | `"Compare these two products: https://a.com/1 and https://b.com/2."` |
| Trend investigation | `"Investigate trends in {technology} over the last 3 months."` |
| Price comparison | `"Create a price comparison table from https://a.com, https://b.com, https://c.com."` |
| Review aggregation | `"Collect user reviews for {product} from 3 sources and calculate positive/negative ratios."` |
| Documentation lookup | `"Find all sections discussing {keyword} on https://docs.example.com."` |
| Blog summary | `"Summarize the latest 5 blog posts from https://blog.example.com."` |
| Live status check | `"Search for the current live status and updates regarding {event}."` |

---

## 🌐 Browser Configuration

### Enable Browser Tools

To allow the agent to navigate the web, enable browser tools in your assistant profile.

1. Open `/assistants` and select your target assistant.
2. Confirm that **Browser** (`browser-sidecar`) or **Web Search** tools are enabled.
3. Save changes if you modified tool settings.

### Recommended System Prompt

For an assistant specialized in web investigation:

```
You are a web research specialist.
Collect real-time web information, verify reliable primary sources, and write structured, objective reports.
Always provide clickable reference URLs for every factual claim.
```

---

## ⚠️ Important Precautions

### Website Access Restrictions

- Some websites block automated scrapers or headless browsers with CAPTCHAs.
- When blocked, the agent reports an access error.
- Copy and paste the web text directly into the chat session as an alternative.

### Information Credibility

- Web information requires independent validation.
- Verify claims against the original source links provided by the agent.
- Cross-reference critical business decisions across multiple independent domains.

### Rate Limits and Bandwidth

- Extensive browsing within a single session can consume significant context tokens.
- For high-volume research, split tasks across multiple sessions or schedule periodic runs.

---

## 🔗 Related Documentation

- [Deep Research Scenario](./research.md) — Synthesize web discoveries into formal reports
- [File Management Scenario](./file-management.md) — Save and organize collected data on disk

---

## 🚀 Next Steps

| Project | Description | Difficulty |
| --- | --- | --- |
| Real-time News Monitor | Receive alerts on specific breaking news topics | ⭐⭐⭐ |
| Automated Price Tracker | Monitor product pricing adjustments across e-commerce sites | ⭐⭐⭐ |
| Review Sentiment Analyzer | Gather customer reviews and evaluate sentiment | ⭐⭐ |
| Competitor Tracker | Detect updates and new releases on competitor websites | ⭐⭐⭐ |
| Web Archive Builder | Save curated web articles into markdown files | ⭐⭐ |
| Web Research Assistant | Build a dedicated web research assistant persona | ⭐⭐ |
