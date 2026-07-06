#!/usr/bin/env python3
"""
Workspace Search Runner
=======================
Executes search queries against the workspace index using BM25, Semantic,
or Hybrid (RRF) search.

Usage:
    python search.py --query "your search term" [--root ROOT_DIR] [--type TYPE]
                     [--top-k K] [--json] [--api-key KEY] [--endpoint ENDPOINT]

Options:
    --query       The search query string (required)
    --root        Workspace root directory (default: current directory)
    --type        Search type: hybrid, bm25, semantic (default: hybrid)
    --top-k       Number of results to return (default: 10)
    --json        Output results in JSON format instead of Markdown
    --api-key     API key to use for query embedding (if different from index config)
    --endpoint    Custom API endpoint for query embedding
"""

import argparse
import json
import math
import os
import sys
from pathlib import Path

# Add script directory to path to import utils
sys.path.append(str(Path(__file__).resolve().parent))
import utils


def run_bm25_search(query: str, bm25_index: dict) -> list[tuple[str, float]]:
    """Calculate BM25 scores for all documents and return ranked (doc_id, score) list."""
    query_tokens = utils.tokenize(query)
    if not query_tokens:
        return []

    doc_count = bm25_index["doc_count"]
    avg_doc_len = bm25_index["avg_doc_len"]
    doc_lengths = bm25_index["doc_lengths"]
    df = bm25_index["df"]
    tf = bm25_index["tf"]

    # BM25 parameters
    k1 = 1.5
    b = 0.75

    scores = {}
    for token in query_tokens:
        # Get Document Frequency for this term
        df_t = df.get(token, 0)
        if df_t == 0:
            continue

        # Calculate Inverse Document Frequency (IDF)
        # IDF = ln((N - df(t) + 0.5) / (df(t) + 0.5) + 1)
        idf = math.log((doc_count - df_t + 0.5) / (df_t + 0.5) + 1.0)

        for doc_id, tf_map in tf.items():
            tf_d_t = tf_map.get(token, 0)
            if tf_d_t == 0:
                continue

            doc_len = doc_lengths.get(doc_id, 0)
            if doc_len == 0:
                continue

            # Score term = idf * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * (doc_len / avg_doc_len)))
            denominator = tf_d_t + k1 * (1.0 - b + b * (doc_len / avg_doc_len))
            term_score = idf * (tf_d_t * (k1 + 1.0)) / denominator
            scores[doc_id] = scores.get(doc_id, 0.0) + term_score

    # Sort documents by score descending
    ranked_docs = sorted(scores.items(), key=lambda x: x[1], reverse=True)
    return ranked_docs


def run_semantic_search(
    query: str,
    semantic_index: dict,
    meta: dict,
    api_key_override: str = None,
    endpoint_override: str = None
) -> list[tuple[str, float]]:
    """Generate embedding for query and compute cosine similarities against indexed chunks."""
    provider = meta.get("provider", "none")
    model = meta.get("model")

    if provider == "none":
        print("[!] Semantic search is disabled or not indexed. Re-run indexer.py with a provider first.", file=sys.stderr)
        return []

    # Get API key from environment if not overridden
    api_key = api_key_override
    if not api_key:
        if provider == "openai":
            api_key = os.environ.get("OPENAI_API_KEY")
        elif provider == "gemini":
            api_key = os.environ.get("GEMINI_API_KEY")

    # Generate query embedding
    print(f"[*] Embedding search query using {provider.upper()} ({model})...", file=sys.stderr)
    query_embeddings = utils.get_embeddings_batch(
        texts=[query],
        provider=provider,
        api_key=api_key,
        model=model,
        endpoint=endpoint_override
    )

    if not query_embeddings or not query_embeddings[0]:
        print("[!] Error generating query embedding.", file=sys.stderr)
        return []

    query_vec = query_embeddings[0]
    embeddings_map = semantic_index.get("embeddings", {})

    similarities = {}
    for doc_id, doc_vec in embeddings_map.items():
        sim = utils.cosine_similarity(query_vec, doc_vec)
        if sim > 0.0:
            similarities[doc_id] = sim

    # Sort documents by similarity descending
    ranked_docs = sorted(similarities.items(), key=lambda x: x[1], reverse=True)
    return ranked_docs


def run_hybrid_search(
    bm25_ranks: list[tuple[str, float]],
    semantic_ranks: list[tuple[str, float]]
) -> list[dict]:
    """Combine BM25 and Semantic search results using Reciprocal Rank Fusion (RRF)."""
    # Create rank maps
    bm25_map = {doc_id: idx + 1 for idx, (doc_id, _) in enumerate(bm25_ranks)}
    semantic_map = {doc_id: idx + 1 for idx, (doc_id, _) in enumerate(semantic_ranks)}

    # Document details from both lists
    bm25_scores = dict(bm25_ranks)
    semantic_scores = dict(semantic_ranks)

    # RRF Constant
    k = 60

    rrf_scores = {}
    all_doc_ids = set(bm25_map.keys()) | set(semantic_map.keys())

    for doc_id in all_doc_ids:
        # Score = Sum ( 1 / (k + rank) )
        score = 0.0
        if doc_id in bm25_map:
            score += 1.0 / (k + bm25_map[doc_id])
        if doc_id in semantic_map:
            score += 1.0 / (k + semantic_map[doc_id])
        rrf_scores[doc_id] = score

    sorted_rrf = sorted(rrf_scores.items(), key=lambda x: x[1], reverse=True)

    results = []
    for doc_id, rrf_score in sorted_rrf:
        results.append({
            "doc_id": doc_id,
            "rrf_score": rrf_score,
            "bm25_rank": bm25_map.get(doc_id, None),
            "bm25_score": bm25_scores.get(doc_id, 0.0),
            "semantic_rank": semantic_map.get(doc_id, None),
            "semantic_score": semantic_scores.get(doc_id, 0.0)
        })

    return results


def main():
    parser = argparse.ArgumentParser(description="Workspace Search Runner")
    parser.add_argument("--query", type=str, required=True, help="Search query string")
    parser.add_argument("--root", type=str, default=".", help="Workspace root directory")
    parser.add_argument("--type", type=str, default="hybrid",
                        choices=["hybrid", "bm25", "semantic"],
                        help="Search execution type")
    parser.add_argument("--top-k", type=int, default=10, help="Number of results to show")
    parser.add_argument("--json", action="store_true", help="Format output as JSON")
    parser.add_argument("--api-key", type=str, default=None, help="Explicit API Key")
    parser.add_argument("--endpoint", type=str, default=None, help="Explicit API Endpoint")
    args = parser.parse_args()

    root_path = Path(args.root).resolve()
    index_dir = root_path / ".libragent" / "search_index"

    meta_path = index_dir / "meta.json"
    bm25_path = index_dir / "bm25_index.json"
    semantic_path = index_dir / "semantic_index.json"

    # 1. Load Indexes
    if not meta_path.exists() or not bm25_path.exists():
        print(f"[!] Search index files missing in {index_dir}. Please run indexer.py first.", file=sys.stderr)
        sys.exit(1)

    try:
        with open(meta_path, "r", encoding="utf-8") as f:
            meta = json.load(f)
        with open(bm25_path, "r", encoding="utf-8") as f:
            bm25_index = json.load(f)
    except Exception as e:
        print(f"[!] Error reading search index: {e}", file=sys.stderr)
        sys.exit(1)

    semantic_index = {}
    if (args.type in ["hybrid", "semantic"]) and semantic_path.exists():
        try:
            with open(semantic_path, "r", encoding="utf-8") as f:
                semantic_index = json.load(f)
        except Exception as e:
            print(f"[!] Error loading semantic index: {e}. Falling back to BM25 search.", file=sys.stderr)
            args.type = "bm25"

    provider = meta.get("provider", "none")
    if provider == "none" and args.type == "semantic":
        print("[!] Semantic index does not exist (provider is none). Run indexer.py first.", file=sys.stderr)
        sys.exit(1)
    elif provider == "none" and args.type == "hybrid":
        # Fallback to pure BM25 if semantic search index is unavailable
        args.type = "bm25"

    # 2. Run searches
    results = []
    
    if args.type == "bm25":
        bm25_ranks = run_bm25_search(args.query, bm25_index)
        for idx, (doc_id, score) in enumerate(bm25_ranks[:args.top_k]):
            results.append({
                "doc_id": doc_id,
                "score": score,
                "type": "bm25",
                "rank": idx + 1
            })

    elif args.type == "semantic":
        semantic_ranks = run_semantic_search(
            args.query, semantic_index, meta, args.api_key, args.endpoint
        )
        for idx, (doc_id, score) in enumerate(semantic_ranks[:args.top_k]):
            results.append({
                "doc_id": doc_id,
                "score": score,
                "type": "semantic",
                "rank": idx + 1
            })

    elif args.type == "hybrid":
        bm25_ranks = run_bm25_search(args.query, bm25_index)
        semantic_ranks = run_semantic_search(
            args.query, semantic_index, meta, args.api_key, args.endpoint
        )
        hybrid_results = run_hybrid_search(bm25_ranks, semantic_ranks)
        for idx, item in enumerate(hybrid_results[:args.top_k]):
            results.append({
                "doc_id": item["doc_id"],
                "score": item["rrf_score"],
                "type": "hybrid",
                "rank": idx + 1,
                "bm25_rank": item["bm25_rank"],
                "bm25_score": item["bm25_score"],
                "semantic_rank": item["semantic_rank"],
                "semantic_score": item["semantic_score"]
            })

    # 3. Assemble document contents and print results
    docs_map = bm25_index.get("docs", {})
    output_data = []

    for item in results:
        doc_id = item["doc_id"]
        doc_info = docs_map.get(doc_id)
        if not doc_info:
            continue

        file_path = doc_info["file_path"]
        start_line = doc_info["start_line"]
        end_line = doc_info["end_line"]
        content = doc_info["content"]

        abs_path = root_path / file_path

        output_data.append({
            "rank": item["rank"],
            "score": item.get("score", 0.0),
            "file_path": file_path,
            "absolute_path": str(abs_path.as_posix()),
            "start_line": start_line,
            "end_line": end_line,
            "content": content,
            "match_details": item
        })

    if args.json:
        print(json.dumps(output_data, ensure_ascii=False, indent=2))
        return

    # Print as beautiful Markdown
    if not output_data:
        print(f"No results found for query: *\"{args.query}\"*")
        return

    print(f"# Workspace Search Results")
    print(f"Query: *\"{args.query}\"* | Search Type: **{args.type.upper()}**")
    print(f"Generated from: {meta.get('last_updated')} (Index provider: {meta.get('provider')})")
    print()

    for item in output_data:
        # Format the file path link for Antigravity GUI constraints
        # Ensure forward slashes for Windows compatibility
        file_url = f"file:///{item['absolute_path']}#L{item['start_line']}-L{item['end_line']}"
        file_name = Path(item['file_path']).name
        
        # Details about the match
        details = item["match_details"]
        if args.type == "hybrid":
            bm25_desc = f"BM25: rank {details['bm25_rank']} (score {details['bm25_score']:.2f})" if details['bm25_rank'] else "BM25: N/A"
            semantic_desc = f"Semantic: rank {details['semantic_rank']} (cosine {details['semantic_score']:.3f})" if details['semantic_rank'] else "Semantic: N/A"
            match_info = f"{bm25_desc} | {semantic_desc}"
        elif args.type == "bm25":
            match_info = f"Score: {item['score']:.4f}"
        else:
            match_info = f"Cosine similarity: {item['score']:.4f}"

        print(f"### {item['rank']}. [{file_name}]({file_url})")
        print(f"- **Path**: `{item['file_path']}` (Lines {item['start_line']}-{item['end_line']})")
        print(f"- **Match Info**: {match_info}")
        print(f"- **Snippet**:")
        # Detect syntax from file extension for styling
        suffix = Path(item['file_path']).suffix.lstrip(".") or ""
        # Handle some typical variations
        if suffix in ["tsx", "jsx"]:
            suffix = "tsx"
        elif suffix in ["ts", "js"]:
            suffix = "typescript"
        elif suffix in ["py"]:
            suffix = "python"
        elif suffix in ["rs"]:
            suffix = "rust"
            
        print(f"  ```{suffix}")
        # Indent content blocks for nice listing
        for line in item['content'].splitlines():
            print(f"  {line}")
        print(f"  ```")
        print()


if __name__ == "__main__":
    main()
