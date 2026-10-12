#!/usr/bin/env python3
"""
Science search CLI tool using ScienceMadeReadable API.
Free, zero-config access to ~200M scholarly papers from OpenAlex.
"""

import argparse
import json
import sys
import urllib.error
import urllib.parse
import urllib.request

# Ensure UTF-8 output on Windows consoles (avoid cp949/cp1252 UnicodeEncodeError)
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8")

BASE_URL = "https://api.sciencemadereadable.com/api/data"


def fetch_url(url: str) -> dict:
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "LibrAgent-ScienceSearch/1.0"},
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = resp.read().decode("utf-8")
            return json.loads(data)
    except urllib.error.HTTPError as e:
        error_body = e.read().decode("utf-8", errors="replace")
        if e.code == 429:
            sys.stderr.write(f"Error 429 (Rate Limit): {error_body}\n")
        else:
            sys.stderr.write(f"HTTP Error {e.code}: {error_body}\n")
        sys.exit(1)
    except Exception as e:
        sys.stderr.write(f"Network error: {e}\n")
        sys.exit(1)


def format_paper_md(paper: dict, include_abstract: bool = True) -> str:
    lines = []
    title = paper.get("title") or "Untitled"
    year = paper.get("year") or "n.d."
    venue = paper.get("venue") or "Unknown Venue"
    citations = paper.get("citations", 0)
    ptype = paper.get("type", "work")
    is_oa = "Open Access" if paper.get("is_oa") else "Closed"
    doi = paper.get("doi")
    oa_url = paper.get("openalex")

    lines.append(f"### {title}")
    authors = paper.get("authors") or ""
    if len(authors) > 120:
        authors = authors[:117] + "..."
    meta = f"**{year}** · *{venue}* · cited {citations} times · {ptype} · {is_oa}"
    lines.append(meta)
    if authors:
        lines.append(f"Authors: {authors}")

    links = []
    if doi:
        links.append(f"DOI: https://doi.org/{doi}")
    if oa_url:
        links.append(f"OpenAlex: {oa_url}")
    if doi:
        links.append(f"Reader: https://sciencemadereadable.com/?work={doi}")
    if links:
        lines.append(" · ".join(links))

    if include_abstract and paper.get("abstract"):
        lines.append(f"\n> {paper['abstract']}\n")
    return "\n".join(lines)


def cmd_search(args) -> None:
    params = {
        "q": args.query,
        "k": str(args.limit),
        "abstracts": "false" if args.no_abstracts else "true",
    }
    if args.year_from:
        params["year_from"] = str(args.year_from)
    if args.year_to:
        params["year_to"] = str(args.year_to)
    if args.type:
        params["type"] = args.type
    if args.oa:
        params["open_access"] = "true"

    qs = urllib.parse.urlencode(params)
    url = f"{BASE_URL}/search?{qs}"
    res = fetch_url(url)

    if args.json:
        print(json.dumps(res, indent=2, ensure_ascii=False))
        return

    mode = res.get("mode", "unknown")
    results = res.get("results", [])
    print(f"## Search Results for: {args.query}")
    print(f"*Mode: {mode} · Found {len(results)} items*\n")

    for i, paper in enumerate(results, 1):
        print(f"#### Result #{i}")
        print(format_paper_md(paper, include_abstract=not args.no_abstracts))
        print("---")


def cmd_work(args) -> None:
    ref = urllib.parse.quote(args.work, safe="")
    url = f"{BASE_URL}/works/{ref}"
    res = fetch_url(url)
    if args.json:
        print(json.dumps(res, indent=2, ensure_ascii=False))
    else:
        print(format_paper_md(res, include_abstract=True))


def cmd_similar(args) -> None:
    ref = urllib.parse.quote(args.similar, safe="")
    params = {
        "k": str(args.limit),
        "abstracts": "false" if args.no_abstracts else "true",
    }
    qs = urllib.parse.urlencode(params)
    url = f"{BASE_URL}/similar/{ref}?{qs}"
    res = fetch_url(url)
    if args.json:
        print(json.dumps(res, indent=2, ensure_ascii=False))
        return

    results = res.get("results", [])
    print(f"## Similar Papers to: {args.similar}")
    print(f"*Found {len(results)} items*\n")
    for i, paper in enumerate(results, 1):
        print(f"#### #{i}")
        print(format_paper_md(paper, include_abstract=not args.no_abstracts))
        print("---")


def cmd_walk(args) -> None:
    ref = urllib.parse.quote(args.walk, safe="")
    url = f"{BASE_URL}/walk?id={ref}&k={args.limit}"
    res = fetch_url(url)
    if args.json:
        print(json.dumps(res, indent=2, ensure_ascii=False))
        return

    answers = res.get("answers", [])
    nodes = res.get("nodes", {})
    print(f"## Citation Walk from: {args.walk}")
    print(f"*Core cited candidates: {len(answers)} items*\n")
    for i, wid in enumerate(answers, 1):
        node = nodes.get(wid, {})
        title = node.get("title", wid)
        year = node.get("year", "")
        cites = node.get("citations", 0)
        topic = node.get("topic", "")
        print(f"{i}. **{title}** ({year}) - Cited {cites} times. Topic: {topic}")
        print(f"   OpenAlex: https://openalex.org/{wid}")


def main():
    parser = argparse.ArgumentParser(
        description="Search ~200M scholarly papers via ScienceMadeReadable API."
    )
    parser.add_argument("query", nargs="?", help="Search query (semantic or keyword)")
    parser.add_argument("--work", "-w", help="Look up a paper by DOI or OpenAlex ID")
    parser.add_argument("--similar", "-s", help="Find papers similar to a DOI or OpenAlex ID")
    parser.add_argument("--walk", help="Citation walk from a DOI or OpenAlex ID")
    parser.add_argument("--limit", "-k", type=int, default=10, help="Result count (1-100)")
    parser.add_argument("--no-abstracts", action="store_true", help="Exclude abstracts to save tokens")
    parser.add_argument("--year-from", type=int, help="Filter: minimum publication year")
    parser.add_argument("--year-to", type=int, help="Filter: maximum publication year")
    parser.add_argument("--type", help="Filter: article, review, preprint, etc.")
    parser.add_argument("--oa", action="store_true", help="Filter: open access papers only")
    parser.add_argument("--json", action="store_true", help="Output raw JSON")

    args = parser.parse_args()

    if args.work:
        cmd_work(args)
    elif args.similar:
        cmd_similar(args)
    elif args.walk:
        cmd_walk(args)
    elif args.query:
        cmd_search(args)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
