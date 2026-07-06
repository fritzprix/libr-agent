#!/usr/bin/env python3
"""
Workspace Search Indexer
=======================
Scans the workspace, extracts text, chunks files, and builds/updates the
BM25 and Semantic search indexes incrementally.

Usage:
    python indexer.py [--root ROOT_DIR] [--provider PROVIDER] [--model MODEL]
                      [--api-key KEY] [--endpoint ENDPOINT] [--rebuild]

Options:
    --root        Workspace root directory (default: current directory)
    --provider    Embedding provider: openai, gemini, ollama, local, none
                  (default: auto-detected based on API keys or sentence-transformers)
    --model       Embedding model to use
    --api-key     API key for the embedding provider
    --endpoint    Custom endpoint URL for the embedding provider
    --rebuild     Force full rebuild of the index (ignores cache)
"""

import argparse
import hashlib
import json
import os
import sys
import time
from datetime import datetime
from pathlib import Path

# Add script directory to path to import utils
sys.path.append(str(Path(__file__).resolve().parent))
import utils

# Default ignore directories
DEFAULT_SKIP_DIRS = {
    ".git", "__pycache__", "node_modules", ".github",
    "venv", ".venv", ".mypy_cache", ".libragent", "attachments", "dist",
    "target", "tmp", "out", ".vscode", ".idea"
}

# Supported file extensions for indexing
TEXT_EXTS = {
    ".md", ".txt", ".py", ".sh", ".json", ".yaml", ".yml", ".toml",
    ".csv", ".html", ".js", ".ts", ".rs", ".tsx", ".jsx", ".java",
    ".cpp", ".h", ".go", ".css", ".xml", ".ini", ".sql"
}


def get_file_hash(path: Path) -> str:
    """Compute the SHA256 hash of a file's content."""
    sha256 = hashlib.sha256()
    try:
        with open(path, "rb") as f:
            while chunk := f.read(8192):
                sha256.update(chunk)
        return sha256.hexdigest()
    except Exception:
        return ""


def get_path_hash(rel_path: str) -> str:
    """Generate a short stable hash for a relative path."""
    return hashlib.sha256(rel_path.encode("utf-8")).hexdigest()[:12]


def detect_provider() -> str:
    """Auto-detect embedding provider based on environment and installed packages."""
    if os.environ.get("OPENAI_API_KEY"):
        return "openai"
    if os.environ.get("GEMINI_API_KEY"):
        return "gemini"
    # Check if sentence-transformers is installed
    try:
        import sentence_transformers
        return "local"
    except ImportError:
        pass
    # Default to none (BM25 only)
    return "none"


def main():
    parser = argparse.ArgumentParser(description="Workspace Search Indexer")
    parser.add_argument("--root", type=str, default=".", help="Workspace root directory")
    parser.add_argument("--provider", type=str, default=None,
                        choices=["openai", "gemini", "ollama", "local", "none"],
                        help="Embedding provider for semantic search")
    parser.add_argument("--model", type=str, default=None, help="Embedding model")
    parser.add_argument("--api-key", type=str, default=None, help="API Key for provider")
    parser.add_argument("--endpoint", type=str, default=None, help="Custom endpoint API URL")
    parser.add_argument("--rebuild", action="store_true", help="Force rebuild of index")
    parser.add_argument("--chunk-size", type=int, default=1200, help="Max chunk size in chars")
    parser.add_argument("--chunk-overlap", type=int, default=300, help="Chunk overlap in chars")
    args = parser.parse_args()

    root_path = Path(args.root).resolve()
    index_dir = root_path / ".libragent" / "search_index"
    index_dir.mkdir(parents=True, exist_ok=True)

    meta_path = index_dir / "meta.json"
    bm25_path = index_dir / "bm25_index.json"
    semantic_path = index_dir / "semantic_index.json"

    # 1. Resolve provider and API key
    provider = args.provider or detect_provider()
    api_key = args.api_key
    if not api_key:
        if provider == "openai":
            api_key = os.environ.get("OPENAI_API_KEY")
        elif provider == "gemini":
            api_key = os.environ.get("GEMINI_API_KEY")

    # Set default models
    model = args.model
    if not model:
        if provider == "openai":
            model = "text-embedding-3-small"
        elif provider == "gemini":
            model = "models/text-embedding-004"
        elif provider == "ollama":
            model = "nomic-embed-text"
        elif provider == "local":
            model = "all-MiniLM-L6-v2"

    print(f"[*] Workspace Root: {root_path}")
    print(f"[*] Search Index Directory: {index_dir}")
    print(f"[*] Semantic Search Provider: {provider.upper()}")
    if provider != "none":
        print(f"[*] Embedding Model: {model}")

    # 2. Load existing indexes
    meta = {"last_updated": "", "files": {}, "provider": provider, "model": model}
    bm25_index = {
        "avg_doc_len": 0.0,
        "doc_count": 0,
        "doc_lengths": {},
        "df": {},
        "tf": {},
        "docs": {}
    }
    semantic_index = {
        "embeddings": {},
        "provider": provider,
        "model": model
    }

    if not args.rebuild and meta_path.exists() and bm25_path.exists():
        try:
            with open(meta_path, "r", encoding="utf-8") as f:
                meta = json.load(f)
            with open(bm25_path, "r", encoding="utf-8") as f:
                bm25_index = json.load(f)
            if semantic_path.exists():
                with open(semantic_path, "r", encoding="utf-8") as f:
                    semantic_index = json.load(f)
            print("[+] Loaded existing indexes successfully.")
        except Exception as e:
            print(f"[!] Error loading existing index, starting fresh: {e}", file=sys.stderr)

    # If the provider/model configuration changed, we must rebuild semantic index or prompt
    if semantic_index.get("provider") != provider or semantic_index.get("model") != model:
        if not args.rebuild and provider != "none":
            print(f"[!] Provider/Model config changed (previously {semantic_index.get('provider')}/{semantic_index.get('model')}). Re-embedding modified files.")
            semantic_index["provider"] = provider
            semantic_index["model"] = model
            semantic_index["embeddings"] = {}
            # Force rebuild to generate embeddings for all current files
            args.rebuild = True
            meta = {"last_updated": "", "files": {}, "provider": provider, "model": model}
            bm25_index = {"avg_doc_len": 0.0, "doc_count": 0, "doc_lengths": {}, "df": {}, "tf": {}, "docs": {}}

    # 3. Scan workspace for files
    print("[*] Scanning workspace files...")
    current_files = {}
    
    for root, dirs, filenames in os.walk(root_path):
        # Prune directories in-place so os.walk doesn't descend into ignored ones
        dirs[:] = [d for d in dirs if d not in DEFAULT_SKIP_DIRS]
        
        for filename in filenames:
            p = Path(root) / filename
            if p.suffix.lower() not in TEXT_EXTS:
                continue
                
            rel_path = str(p.relative_to(root_path).as_posix())
            mtime = p.stat().st_mtime
            f_hash = get_file_hash(p)
            if f_hash:
                current_files[rel_path] = {"hash": f_hash, "mtime": mtime}

    # 4. Compare and determine changes
    old_files = meta.get("files", {})
    
    deleted_files = []
    for rel_path in old_files:
        if rel_path not in current_files:
            deleted_files.append(rel_path)

    modified_files = []
    for rel_path, info in current_files.items():
        if rel_path not in old_files:
            modified_files.append(rel_path)
        elif old_files[rel_path].get("hash") != info["hash"]:
            modified_files.append(rel_path)

    print(f"[*] Found {len(current_files)} indexable files.")
    print(f"[*] Changes: {len(modified_files)} modified/new, {len(deleted_files)} deleted.")

    if not modified_files and not deleted_files:
        print("[+] Search index is already up-to-date.")
        return

    # 5. Apply changes incrementally
    # First: remove deleted files
    for rel_path in deleted_files:
        print(f"[-] Removing deleted file from index: {rel_path}")
        path_hash = get_path_hash(rel_path)
        # Find all doc_ids belonging to this file
        doc_ids_to_remove = [
            doc_id for doc_id, doc_info in bm25_index["docs"].items()
            if doc_info["file_path"] == rel_path
        ]
        for doc_id in doc_ids_to_remove:
            bm25_index["docs"].pop(doc_id, None)
            bm25_index["tf"].pop(doc_id, None)
            bm25_index["doc_lengths"].pop(doc_id, None)
            semantic_index["embeddings"].pop(doc_id, None)

    # Second: process modified and new files
    chunks_to_embed = []
    chunk_ids_to_embed = []

    for rel_path in modified_files:
        print(f"[+] Processing file: {rel_path}")
        path_hash = get_path_hash(rel_path)
        
        # Clean up any existing chunks for this modified file first
        doc_ids_to_remove = [
            doc_id for doc_id, doc_info in bm25_index["docs"].items()
            if doc_info["file_path"] == rel_path
        ]
        for doc_id in doc_ids_to_remove:
            bm25_index["docs"].pop(doc_id, None)
            bm25_index["tf"].pop(doc_id, None)
            bm25_index["doc_lengths"].pop(doc_id, None)
            semantic_index["embeddings"].pop(doc_id, None)

        # Chunk the new content
        file_absolute = root_path / rel_path
        try:
            text_content = file_absolute.read_text(encoding="utf-8", errors="ignore")
        except Exception as e:
            print(f"[!] Failed to read {rel_path}: {e}", file=sys.stderr)
            continue

        chunks = utils.chunk_text(text_content, args.chunk_size, args.chunk_overlap)
        current_files[rel_path]["chunks_count"] = len(chunks)

        for idx, chunk in enumerate(chunks):
            doc_id = f"doc_{path_hash}_{idx}"
            content = chunk["content"]
            
            # Save doc details
            bm25_index["docs"][doc_id] = {
                "file_path": rel_path,
                "start_line": chunk["start_line"],
                "end_line": chunk["end_line"],
                "content": content
            }
            
            # Compute TF
            tokens = utils.tokenize(content)
            bm25_index["doc_lengths"][doc_id] = len(tokens)
            
            tf = {}
            for t in tokens:
                tf[t] = tf.get(t, 0) + 1
            bm25_index["tf"][doc_id] = tf

            # Prepare for semantic search
            if provider != "none":
                chunks_to_embed.append(content)
                chunk_ids_to_embed.append(doc_id)

    # 6. Generate embeddings if semantic search is active
    if provider != "none" and chunks_to_embed:
        print(f"[*] Generating embeddings for {len(chunks_to_embed)} chunks in batches...")
        batch_size = 20
        total_batches = (len(chunks_to_embed) + batch_size - 1) // batch_size
        
        for b in range(total_batches):
            start = b * batch_size
            end = min(start + batch_size, len(chunks_to_embed))
            batch_texts = chunks_to_embed[start:end]
            batch_ids = chunk_ids_to_embed[start:end]
            
            print(f"    Batch {b+1}/{total_batches} ({len(batch_texts)} chunks)...")
            
            # Try to get embeddings
            embeddings = utils.get_embeddings_batch(
                texts=batch_texts,
                provider=provider,
                api_key=api_key,
                model=model,
                endpoint=args.endpoint
            )
            
            if embeddings:
                for doc_id, vec in zip(batch_ids, embeddings):
                    if vec:
                        semantic_index["embeddings"][doc_id] = vec
            else:
                print(f"[!] Warning: Failed to obtain embeddings for batch {b+1}. Skipping these chunks in semantic search.", file=sys.stderr)
            
            # Polite sleep to avoid rate limits
            if b < total_batches - 1 and provider in ["openai", "gemini"]:
                time.sleep(0.5)

    # 7. Recompute global BM25 metrics
    print("[*] Recomputing global BM25 statistics...")
    doc_count = len(bm25_index["docs"])
    bm25_index["doc_count"] = doc_count
    
    total_len = sum(bm25_index["doc_lengths"].values())
    bm25_index["avg_doc_len"] = total_len / doc_count if doc_count > 0 else 0.0

    # Recompute DF
    df = {}
    for doc_id, tf_map in bm25_index["tf"].items():
        for term in tf_map:
            df[term] = df.get(term, 0) + 1
    bm25_index["df"] = df

    # 8. Save updated indexes
    # Keep track of active file metadata
    active_files = {}
    for rel_path, info in current_files.items():
        # Keep it in active if it actually got processed or exists
        if rel_path in current_files:
            active_files[rel_path] = info
            
    meta["files"] = active_files
    meta["last_updated"] = datetime.now().isoformat()
    meta["provider"] = provider
    meta["model"] = model

    with open(meta_path, "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2)
    with open(bm25_path, "w", encoding="utf-8") as f:
        json.dump(bm25_index, f, indent=2)
    
    if provider != "none":
        with open(semantic_path, "w", encoding="utf-8") as f:
            json.dump(semantic_index, f, indent=2)
    elif semantic_path.exists():
        # If semantic is none, delete semantic index or clear embeddings
        semantic_index["embeddings"] = {}
        with open(semantic_path, "w", encoding="utf-8") as f:
            json.dump(semantic_index, f, indent=2)

    print(f"[+] Indexing complete. Total active chunks: {doc_count}.")


if __name__ == "__main__":
    main()
