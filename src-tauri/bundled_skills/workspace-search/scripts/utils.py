#!/usr/bin/env python3
"""
Workspace Search Utilities
=========================
Helper functions for tokenization, chunking, calling API embedding endpoints
(OpenAI, Gemini, Ollama), local embedding generation, and cosine similarity.
"""

import io
import json
import re
import sys
import urllib.request
import urllib.error
from pathlib import Path

# Prevent UnicodeEncodeError on Windows
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")


def tokenize(text: str) -> list[str]:
    """Tokenize text into lowercase alphanumeric words."""
    return re.findall(r'[a-zA-Z0-9가-힣]+', text.lower())


def chunk_text(text: str, chunk_size: int = 1200, chunk_overlap: int = 300) -> list[dict]:
    """
    Split text into overlapping chunks, tracking line numbers.
    Preserves line boundaries. Each chunk is:
    {
        'content': str,
        'start_line': int,
        'end_line': int
    }
    """
    lines = text.splitlines()
    if not lines:
        return []

    chunks = []
    current_chunk_lines = []
    current_chunk_chars = 0
    start_line = 1

    for i, line in enumerate(lines, 1):
        current_chunk_lines.append(line)
        current_chunk_chars += len(line) + 1  # +1 for newline

        if current_chunk_chars >= chunk_size:
            content = "\n".join(current_chunk_lines)
            chunks.append({
                'content': content,
                'start_line': start_line,
                'end_line': i
            })

            # Calculate overlap backtracking
            overlap_chars = 0
            overlap_lines = []
            for rev_line in reversed(current_chunk_lines):
                overlap_chars += len(rev_line) + 1
                overlap_lines.insert(0, rev_line)
                if overlap_chars >= chunk_overlap:
                    break

            current_chunk_lines = overlap_lines
            current_chunk_chars = overlap_chars
            start_line = i - len(current_chunk_lines) + 1

    # Add remaining text
    if current_chunk_lines:
        content = "\n".join(current_chunk_lines)
        chunks.append({
            'content': content,
            'start_line': start_line,
            'end_line': len(lines)
        })

    return chunks


def get_embeddings_batch(
    texts: list[str],
    provider: str,
    api_key: str = None,
    model: str = None,
    endpoint: str = None
) -> list[list[float]] | None:
    """
    Call embedding API for a batch of text chunks.
    Zero-dependency implementation using urllib.request.
    """
    if not texts:
        return []

    provider = provider.lower()

    if provider == "openai":
        url = endpoint or "https://api.openai.com/v1/embeddings"
        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}"
        }
        data = {
            "input": texts,
            "model": model or "text-embedding-3-small"
        }
        req = urllib.request.Request(url, data=json.dumps(data).encode("utf-8"), headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=30) as response:
                res = json.loads(response.read().decode("utf-8"))
                embeddings = [None] * len(texts)
                for item in res.get("data", []):
                    embeddings[item["index"]] = item["embedding"]
                return embeddings
        except Exception as e:
            print(f"OpenAI embedding error: {e}", file=sys.stderr)
            return None

    elif provider == "gemini":
        # Gemini batchEmbedContents
        key_param = f"?key={api_key}" if api_key else ""
        url = endpoint or f"https://generativelanguage.googleapis.com/v1beta/models/batchEmbedContents{key_param}"
        headers = {
            "Content-Type": "application/json"
        }
        gemini_model = model or "models/text-embedding-004"
        if not gemini_model.startswith("models/"):
            gemini_model = f"models/{gemini_model}"

        requests_list = []
        for t in texts:
            requests_list.append({
                "model": gemini_model,
                "content": {"parts": [{"text": t}]}
            })
        data = {"requests": requests_list}
        req = urllib.request.Request(url, data=json.dumps(data).encode("utf-8"), headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=30) as response:
                res = json.loads(response.read().decode("utf-8"))
                embeddings = []
                for item in res.get("embeddings", []):
                    embeddings.append(item.get("values", []))
                return embeddings
        except Exception as e:
            print(f"Gemini embedding error: {e}", file=sys.stderr)
            return None

    elif provider == "ollama":
        url = endpoint or "http://localhost:11434/api/embed"
        data = {
            "model": model or "nomic-embed-text",
            "input": texts
        }
        headers = {
            "Content-Type": "application/json"
        }
        req = urllib.request.Request(url, data=json.dumps(data).encode("utf-8"), headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=30) as response:
                res = json.loads(response.read().decode("utf-8"))
                return res.get("embeddings", [])
        except Exception as e:
            # Fallback to legacy /api/embeddings one by one
            legacy_url = (endpoint or "http://localhost:11434").replace("/api/embed", "/api/embeddings")
            if not legacy_url.endswith("/api/embeddings"):
                legacy_url = legacy_url.rstrip("/") + "/api/embeddings"

            embeddings = []
            for t in texts:
                legacy_data = {"model": model or "nomic-embed-text", "prompt": t}
                req_leg = urllib.request.Request(legacy_url, data=json.dumps(legacy_data).encode("utf-8"), headers=headers, method="POST")
                try:
                    with urllib.request.urlopen(req_leg, timeout=10) as resp_leg:
                        res_leg = json.loads(resp_leg.read().decode("utf-8"))
                        embeddings.append(res_leg.get("embedding", []))
                except Exception as ex:
                    print(f"Ollama legacy embedding error: {ex}", file=sys.stderr)
                    return None
            return embeddings

    elif provider == "local":
        return get_local_embeddings(texts, model or "all-MiniLM-L6-v2")

    return None


def get_local_embeddings(texts: list[str], model_name: str = "all-MiniLM-L6-v2") -> list[list[float]] | None:
    """Generate local embeddings using sentence-transformers (must be installed)."""
    try:
        from sentence_transformers import SentenceTransformer
        # SentenceTransformer handles model downloads and caching automatically
        model = SentenceTransformer(model_name)
        embeddings = model.encode(texts, convert_to_numpy=True).tolist()
        return embeddings
    except ImportError:
        print("sentence-transformers is not installed. Run install_deps.py.", file=sys.stderr)
        return None


def cosine_similarity(v1: list[float], v2: list[float]) -> float:
    """Compute cosine similarity between two float vectors."""
    if not v1 or not v2 or len(v1) != len(v2):
        return 0.0
    dot_product = sum(a * b for a, b in zip(v1, v2))
    norm_a = sum(a * a for a in v1) ** 0.5
    norm_b = sum(b * b for b in v2) ** 0.5
    if norm_a == 0.0 or norm_b == 0.0:
        return 0.0
    return dot_product / (norm_a * norm_b)
