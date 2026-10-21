"""
GitHub skill fetch, selection, script/executable review, and provenance helpers for deploy_skill.py.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import stat
import subprocess
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any

PROVENANCE_FILE_NAME = ".skill-provenance.json"
MAX_GITHUB_ARCHIVE_BYTES = 100 * 1024 * 1024
HTTP_TIMEOUT_SECS = 60
GIT_TIMEOUT_SECS = 20
GIT_FETCH_TIMEOUT_SECS = 120
COMMIT_SHA_RE = re.compile(r"^[0-9a-fA-F]{40}$")
SAFE_SEGMENT_RE = re.compile(r"^[A-Za-z0-9._-]+$")

SCRIPT_OR_EXEC_EXTENSIONS = frozenset(
    {
        ".py",
        ".pyw",
        ".sh",
        ".bash",
        ".zsh",
        ".fish",
        ".ps1",
        ".psm1",
        ".bat",
        ".cmd",
        ".js",
        ".mjs",
        ".cjs",
        ".ts",
        ".mts",
        ".cts",
        ".rb",
        ".pl",
        ".php",
        ".lua",
        ".exe",
        ".dll",
        ".so",
        ".dylib",
        ".bin",
        ".jar",
        ".wasm",
    }
)

NON_EXECUTABLE_DOC_EXTENSIONS = frozenset(
    {
        ".md",
        ".txt",
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".ini",
        ".cfg",
        ".xml",
        ".xsd",
        ".html",
        ".css",
        ".svg",
        ".png",
        ".jpg",
        ".jpeg",
        ".gif",
        ".webp",
        ".ico",
        ".pdf",
        ".docx",
        ".pptx",
        ".xlsx",
        ".csv",
        ".tsv",
    }
)

IGNORED_WALK_DIRS = frozenset(
    {
        ".git",
        ".github",
        ".gitlab",
        ".circleci",
        ".vscode",
        ".idea",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        "node_modules",
        ".venv",
        "venv",
    }
)

# Must remain a superset of FORBIDDEN_SKILL_FILES (lowercased) in skill-creator/scripts/validate_skill.py
STRIPPED_SKILL_ROOT_FILES_LOWER = frozenset(
    {
        ".bundled_manifest.json",
        "readme.md",
        "readme.txt",
        "readme",
        "changelog.md",
        "installation.md",
        "installation_guide.md",
        ".gitignore",
        ".gitattributes",
        ".gitmodules",
        ".editorconfig",
        ".ds_store",
    }
)

IGNORED_SKILL_INTERNAL_FILES = frozenset({PROVENANCE_FILE_NAME, ".force_update", ".DS_Store"})


class GitHubDeployError(Exception):
    def __init__(self, code: str, message: str, fix: str, exit_code: int = 1) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.fix = fix
        self.exit_code = exit_code

    def format(self) -> str:
        return f"[{self.code}] ERROR: {self.message}\n  Fix: {self.fix}"


@dataclass(frozen=True)
class ParsedGitHubUrl:
    requested_url: str
    owner: str
    repo: str
    explicit_ref: str | None
    explicit_subpath: str | None
    tree_segments: tuple[str, ...]

    @property
    def repo_url(self) -> str:
        return f"https://github.com/{self.owner}/{self.repo}"


@dataclass(frozen=True)
class ResolvedGitHubSpec:
    requested_url: str
    owner: str
    repo: str
    ref: str
    commit: str
    subpath: str | None

    @property
    def repo_url(self) -> str:
        return f"https://github.com/{self.owner}/{self.repo}"


def is_github_source(raw: str) -> bool:
    trimmed = raw.strip()
    if not trimmed.startswith(("https://", "http://")):
        return False
    try:
        parsed = urllib.parse.urlparse(trimmed)
    except ValueError:
        return False
    return (parsed.hostname or "").lower() in {"github.com", "www.github.com"}


def is_commit_sha(value: str | None) -> bool:
    return bool(value and COMMIT_SHA_RE.match(value.strip()))


def normalize_relative_subpath(raw_subpath: str | None) -> str | None:
    if raw_subpath is None:
        return None
    cleaned = raw_subpath.strip().replace("\\", "/").strip("/")
    if not cleaned or cleaned == ".":
        return None

    posix_path = PurePosixPath(cleaned)
    if posix_path.is_absolute():
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"GitHub subpath must be relative, got '{raw_subpath}'.",
            "Pass a relative folder path inside the repository (e.g. skills/my-skill).",
            exit_code=2,
        )

    parts: list[str] = []
    for part in posix_path.parts:
        if part in {"", "."}:
            continue
        if part == "..":
            raise GitHubDeployError(
                "DEPLOY-GITHUB-001",
                f"GitHub subpath cannot contain '..': '{raw_subpath}'.",
                "Pass a subpath inside the repository without parent directory traversal.",
                exit_code=2,
            )
        parts.append(part)

    return "/".join(parts) if parts else None


def parse_github_url(
    raw_url: str,
    ref_override: str | None = None,
    subpath_override: str | None = None,
) -> ParsedGitHubUrl:
    trimmed = raw_url.strip()
    try:
        parsed = urllib.parse.urlparse(trimmed)
    except ValueError as exc:
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"Invalid GitHub URL '{raw_url}': {exc}",
            "Pass a valid https://github.com/<owner>/<repo> URL.",
            exit_code=2,
        ) from exc

    if parsed.scheme not in {"https", "http"}:
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"Unsupported URL scheme '{parsed.scheme}'.",
            "Use https://github.com/<owner>/<repo>.",
            exit_code=2,
        )

    host = (parsed.hostname or "").lower()
    if host not in {"github.com", "www.github.com"}:
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"Only github.com URLs are supported (got host '{host or '<empty>'}').",
            "Pass a https://github.com/<owner>/<repo> URL.",
            exit_code=2,
        )

    segments = [
        urllib.parse.unquote(seg)
        for seg in parsed.path.split("/")
        if seg and seg != "."
    ]
    if len(segments) < 2:
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"GitHub URL must include <owner>/<repo>: '{raw_url}'.",
            "Use https://github.com/<owner>/<repo> or https://github.com/<owner>/<repo>/tree/<ref>/<subpath>.",
            exit_code=2,
        )

    owner = segments[0]
    repo = segments[1].removesuffix(".git")
    if (
        not owner
        or not repo
        or owner in {".", ".."}
        or repo in {".", ".."}
        or not SAFE_SEGMENT_RE.match(owner)
        or not SAFE_SEGMENT_RE.match(repo)
    ):
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"Invalid repository owner/name in GitHub URL: '{owner}/{repo}'.",
            "Use https://github.com/<owner>/<repo>.",
            exit_code=2,
        )

    query_pairs = urllib.parse.parse_qs(parsed.query, keep_blank_values=False)
    query_ref = query_pairs.get("ref", [None])[0]
    query_path = query_pairs.get("path", [None])[0]

    if ref_override and query_ref and ref_override.strip() != query_ref.strip():
        raise GitHubDeployError(
            "DEPLOY-GITHUB-001",
            f"Conflicting ref values: --ref '{ref_override.strip()}' vs URL query ?ref='{query_ref.strip()}'.",
            "Specify the ref in only one place.",
            exit_code=2,
        )

    explicit_ref = (ref_override or query_ref or "").strip() or None
    explicit_subpath = normalize_relative_subpath(
        subpath_override if subpath_override is not None else query_path
    )
    tree_segments: tuple[str, ...] = ()

    if len(segments) > 2:
        kind = segments[2]
        rest = segments[3:]
        if kind in {"tree", "blob"}:
            if not rest:
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-001",
                    f"GitHub {kind} URL is missing a branch/tag/commit ref: '{raw_url}'.",
                    "Use https://github.com/<owner>/<repo>/tree/<ref>[/<subpath>].",
                    exit_code=2,
                )
            if kind == "blob":
                if rest[-1] == "SKILL.md":
                    rest = rest[:-1]
                else:
                    raise GitHubDeployError(
                        "DEPLOY-GITHUB-001",
                        f"GitHub blob URL must point to SKILL.md or use /tree/ for a folder: '{raw_url}'.",
                        "Pass the skill folder URL (/tree/<ref>/<path>) or the SKILL.md blob URL.",
                        exit_code=2,
                    )
            if explicit_ref is not None:
                ref_parts = [p for p in explicit_ref.split("/") if p]
                if rest[: len(ref_parts)] != ref_parts:
                    raise GitHubDeployError(
                        "DEPLOY-GITHUB-001",
                        f"Conflicting --ref '{explicit_ref}' with GitHub {kind} URL '{raw_url}'.",
                        "Either omit --ref to use the ref in the /tree/ URL, or pass the repository root URL "
                        f"(https://github.com/{owner}/{repo}) with --ref and --subpath.",
                        exit_code=2,
                    )
                rem = rest[len(ref_parts) :]
                url_subpath = normalize_relative_subpath("/".join(rem)) if rem else None
                if explicit_subpath and url_subpath and explicit_subpath != url_subpath:
                    raise GitHubDeployError(
                        "DEPLOY-GITHUB-001",
                        f"Conflicting --subpath '{explicit_subpath}' with URL subpath '{url_subpath}'.",
                        "Specify the subpath in only one place.",
                        exit_code=2,
                    )
                if explicit_subpath is None:
                    explicit_subpath = url_subpath
            else:
                tree_segments = tuple(rest)
        elif kind == "commit":
            if not rest:
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-001",
                    f"GitHub commit URL is missing a commit SHA: '{raw_url}'.",
                    "Use https://github.com/<owner>/<repo>/commit/<sha>.",
                    exit_code=2,
                )
            url_sha = rest[0]
            if explicit_ref is not None and explicit_ref.lower() != url_sha.lower():
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-001",
                    f"Conflicting --ref '{explicit_ref}' with GitHub commit URL '{raw_url}'.",
                    "Omit --ref when passing a /commit/<sha> URL, or pass the repository root URL.",
                    exit_code=2,
                )
            explicit_ref = url_sha
            if len(rest) > 1:
                url_subpath = normalize_relative_subpath("/".join(rest[1:]))
                if explicit_subpath and url_subpath and explicit_subpath != url_subpath:
                    raise GitHubDeployError(
                        "DEPLOY-GITHUB-001",
                        f"Conflicting --subpath '{explicit_subpath}' with URL subpath '{url_subpath}'.",
                        "Specify the subpath in only one place.",
                        exit_code=2,
                    )
                if explicit_subpath is None:
                    explicit_subpath = url_subpath
        else:
            raise GitHubDeployError(
                "DEPLOY-GITHUB-001",
                f"Unsupported GitHub URL path segment '{kind}' in '{raw_url}'.",
                "Use https://github.com/<owner>/<repo> or https://github.com/<owner>/<repo>/tree/<ref>/<subpath>.",
                exit_code=2,
            )

    return ParsedGitHubUrl(
        requested_url=trimmed,
        owner=owner,
        repo=repo,
        explicit_ref=explicit_ref,
        explicit_subpath=explicit_subpath,
        tree_segments=tree_segments,
    )


def _github_headers() -> dict[str, str]:
    headers = {
        "User-Agent": "LibrAgent-Skill-Deployer",
        "Accept": "application/vnd.github+json",
    }
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token.strip()}"
    return headers


def query_git_ls_remote(repo_url: str) -> tuple[str | None, str | None, dict[str, str]]:
    """
    Return (head_sha, default_branch, refs_map) using `git ls-remote --symref`.
    refs_map maps branch names, tag names, and full ref names to 40-char lowercase commit SHAs.
    """
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"
    try:
        proc = subprocess.run(
            ["git", "ls-remote", "--symref", f"{repo_url}.git"],
            capture_output=True,
            text=True,
            timeout=GIT_TIMEOUT_SECS,
            env=env,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None, None, {}

    if proc.returncode != 0:
        return None, None, {}

    head_sha: str | None = None
    default_branch: str | None = None
    refs_map: dict[str, str] = {}
    peeled_tags: dict[str, str] = {}

    for raw_line in proc.stdout.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if line.startswith("ref: ") and "\tHEAD" in line:
            symref = line.split("\t", 1)[0].removeprefix("ref: ").strip()
            if symref.startswith("refs/heads/"):
                default_branch = symref.removeprefix("refs/heads/")
            continue

        parts = line.split("\t", 1)
        if len(parts) != 2:
            continue
        sha, ref_name = parts[0].strip().lower(), parts[1].strip()
        if not COMMIT_SHA_RE.match(sha):
            continue

        if ref_name == "HEAD":
            head_sha = sha
            refs_map["HEAD"] = sha
        elif ref_name.startswith("refs/heads/"):
            branch = ref_name.removeprefix("refs/heads/")
            refs_map[branch] = sha
            refs_map[ref_name] = sha
        elif ref_name.startswith("refs/tags/"):
            if ref_name.endswith("^{}"):
                tag = ref_name.removeprefix("refs/tags/")[:-3]
                peeled_tags[tag] = sha
                refs_map[ref_name[:-3]] = sha
            else:
                tag = ref_name.removeprefix("refs/tags/")
                refs_map.setdefault(tag, sha)
                refs_map.setdefault(ref_name, sha)

    refs_map.update(peeled_tags)
    if default_branch is None and head_sha:
        for key, sha in refs_map.items():
            if not key.startswith("refs/") and key != "HEAD" and sha == head_sha:
                default_branch = key
                break

    return head_sha, default_branch, refs_map


def query_github_api_commit(owner: str, repo: str, ref: str | None) -> tuple[str, str]:
    """
    Resolve (ref_name, 40_char_commit_sha) via the GitHub REST API.
    """
    resolved_ref = ref
    if not resolved_ref:
        repo_api = f"https://api.github.com/repos/{owner}/{repo}"
        req = urllib.request.Request(repo_api, headers=_github_headers())
        try:
            with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECS) as resp:
                repo_info = json.loads(resp.read().decode("utf-8"))
                resolved_ref = repo_info.get("default_branch") or "HEAD"
        except (OSError, urllib.error.URLError, ValueError, json.JSONDecodeError) as exc:
            raise GitHubDeployError(
                "DEPLOY-GITHUB-002",
                f"Failed to resolve default branch for {owner}/{repo}: {exc}",
                "Check the repository URL, network connection, or pass --ref <branch-or-commit>.",
            ) from exc

    encoded_ref = urllib.parse.quote(resolved_ref, safe="")
    commit_api = f"https://api.github.com/repos/{owner}/{repo}/commits/{encoded_ref}"
    req = urllib.request.Request(commit_api, headers=_github_headers())
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECS) as resp:
            commit_info = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        raise GitHubDeployError(
            "DEPLOY-GITHUB-002",
            f"GitHub could not resolve ref '{resolved_ref}' in {owner}/{repo} (HTTP {exc.code}).",
            "Verify the repository, branch/tag/commit ref, or pass ?ref=<branch>&path=<subpath>.",
        ) from exc
    except (OSError, urllib.error.URLError, ValueError, json.JSONDecodeError) as exc:
        raise GitHubDeployError(
            "DEPLOY-GITHUB-002",
            f"Failed to resolve commit for {owner}/{repo}@{resolved_ref}: {exc}",
            "Check network access to github.com or pass a full 40-character commit SHA via --ref.",
        ) from exc

    sha = str(commit_info.get("sha", "")).strip().lower()
    if not COMMIT_SHA_RE.match(sha):
        raise GitHubDeployError(
            "DEPLOY-GITHUB-002",
            f"GitHub API did not return a valid 40-character commit SHA for {owner}/{repo}@{resolved_ref}.",
            "Pass an explicit branch, tag, or 40-character commit SHA with --ref.",
        )
    return resolved_ref, sha


def resolve_github_spec(parsed: ParsedGitHubUrl) -> ResolvedGitHubSpec:
    head_sha, default_branch, refs_map = query_git_ls_remote(parsed.repo_url)

    ref = parsed.explicit_ref
    subpath = parsed.explicit_subpath
    commit: str | None = None

    if parsed.tree_segments:
        matched_len = 0
        matched_ref: str | None = None
        if refs_map:
            for idx in range(len(parsed.tree_segments), 0, -1):
                candidate_ref = "/".join(parsed.tree_segments[:idx])
                if candidate_ref in refs_map:
                    matched_ref = candidate_ref
                    matched_len = idx
                    commit = refs_map[candidate_ref]
                    break

        if matched_ref is not None:
            ref = matched_ref
            rem = parsed.tree_segments[matched_len:]
        else:
            ref = parsed.tree_segments[0]
            rem = parsed.tree_segments[1:]

        url_subpath = normalize_relative_subpath("/".join(rem)) if rem else None
        if subpath and url_subpath and subpath != url_subpath:
            raise GitHubDeployError(
                "DEPLOY-GITHUB-001",
                f"Conflicting --subpath '{subpath}' with URL subpath '{url_subpath}'.",
                "Specify the subpath in only one place.",
                exit_code=2,
            )
        if subpath is None:
            subpath = url_subpath

    if ref is None:
        if head_sha:
            ref = default_branch or "HEAD"
            commit = head_sha
        else:
            ref, commit = query_github_api_commit(parsed.owner, parsed.repo, None)
    elif commit is None:
        if COMMIT_SHA_RE.match(ref):
            commit = ref.lower()
        elif ref in refs_map:
            commit = refs_map[ref]
        else:
            _, commit = query_github_api_commit(parsed.owner, parsed.repo, ref)

    return ResolvedGitHubSpec(
        requested_url=parsed.requested_url,
        owner=parsed.owner,
        repo=parsed.repo,
        ref=ref,
        commit=commit.lower(),
        subpath=subpath,
    )


def sanitize_staged_skill_dir(skill_dir: Path) -> list[str]:
    """
    Remove repository-level auxiliary docs (README.md, CHANGELOG.md, etc.) and CI/IDE
    directories (.github/, .vscode/, etc.) from a staged skill folder so single-skill
    GitHub repositories pass strict skill layout validation cleanly.
    """
    removed: list[str] = []
    if not skill_dir.is_dir():
        return removed

    for child in list(skill_dir.iterdir()):
        name_lower = child.name.lower()
        if child.is_dir() and not child.is_symlink():
            if child.name in IGNORED_WALK_DIRS or name_lower in IGNORED_WALK_DIRS:
                shutil.rmtree(child)
                removed.append(f"{child.name}/")
        elif (
            child.is_file() or child.is_symlink()
        ) and name_lower in STRIPPED_SKILL_ROOT_FILES_LOWER:
            child.unlink()
            removed.append(child.name)

    return sorted(removed)


def _safe_extract_zip(archive_path: Path, extract_dir: Path) -> None:
    extract_root = extract_dir.resolve()
    with zipfile.ZipFile(archive_path, "r") as zf:
        for member in zf.infolist():
            raw_name = member.filename.replace("\\", "/")
            if not raw_name or raw_name.startswith("/"):
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-004",
                    f"Archive contains unsafe absolute path: '{member.filename}'.",
                    "Refusing to extract untrusted archive.",
                )
            posix_parts = PurePosixPath(raw_name).parts
            if any(part == ".." for part in posix_parts):
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-004",
                    f"Archive contains path traversal entry: '{member.filename}'.",
                    "Refusing to extract untrusted archive.",
                )

            unix_mode = member.external_attr >> 16
            if stat.S_ISLNK(unix_mode):
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-004",
                    f"Archive contains symlink entry '{member.filename}', which is not allowed.",
                    "Remove symlinks from the skill repository before deploying.",
                )

            target_path = (extract_root / Path(*posix_parts)).resolve()
            if target_path != extract_root and extract_root not in target_path.parents:
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-004",
                    f"Archive entry escapes extraction root: '{member.filename}'.",
                    "Refusing to extract untrusted archive.",
                )

            if member.is_dir() or raw_name.endswith("/"):
                target_path.mkdir(parents=True, exist_ok=True)
                continue

            target_path.parent.mkdir(parents=True, exist_ok=True)
            with zf.open(member, "r") as src, target_path.open("wb") as dst:
                shutil.copyfileobj(src, dst)

            if unix_mode & 0o111:
                try:
                    current_mode = target_path.stat().st_mode
                    target_path.chmod(current_mode | 0o111)
                except OSError:
                    pass


def _download_commit_zip(spec: ResolvedGitHubSpec, archive_path: Path) -> None:
    archive_url = f"https://codeload.github.com/{spec.owner}/{spec.repo}/zip/{spec.commit}"
    req = urllib.request.Request(archive_url, headers=_github_headers())
    with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECS) as resp:
        content_len = resp.headers.get("Content-Length")
        if content_len and int(content_len) > MAX_GITHUB_ARCHIVE_BYTES:
            raise GitHubDeployError(
                "DEPLOY-GITHUB-004",
                f"GitHub archive exceeds maximum size ({MAX_GITHUB_ARCHIVE_BYTES // (1024 * 1024)} MB).",
                "Use a smaller repository.",
            )

        downloaded = 0
        with archive_path.open("wb") as out:
            while True:
                chunk = resp.read(65536)
                if not chunk:
                    break
                downloaded += len(chunk)
                if downloaded > MAX_GITHUB_ARCHIVE_BYTES:
                    raise GitHubDeployError(
                        "DEPLOY-GITHUB-004",
                        f"GitHub archive exceeded {MAX_GITHUB_ARCHIVE_BYTES // (1024 * 1024)} MB limit during download.",
                        "Use a smaller repository.",
                    )
                out.write(chunk)


def _git_fetch_commit_fallback(spec: ResolvedGitHubSpec, repo_dest: Path) -> None:
    repo_dest.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"

    commands = [
        ["git", "init", "-q"],
        ["git", "remote", "add", "origin", f"{spec.repo_url}.git"],
        ["git", "fetch", "--depth", "1", "origin", spec.commit],
        ["git", "checkout", "-q", "--detach", "FETCH_HEAD"],
    ]
    for cmd in commands:
        proc = subprocess.run(
            cmd,
            cwd=str(repo_dest),
            capture_output=True,
            text=True,
            timeout=GIT_FETCH_TIMEOUT_SECS,
            env=env,
            check=False,
        )
        if proc.returncode != 0:
            stderr = (proc.stderr or proc.stdout or "").strip()
            raise GitHubDeployError(
                "DEPLOY-GITHUB-004",
                f"Failed to fetch {spec.repo_url} at commit {spec.commit}: {stderr}",
                "Verify the repository URL, commit SHA, and network access.",
            )

    git_dir = repo_dest / ".git"
    if git_dir.exists():
        shutil.rmtree(git_dir)


def fetch_github_repo_at_commit(spec: ResolvedGitHubSpec, temp_dir: Path) -> Path:
    """
    Download and extract the GitHub repository at the pinned commit SHA into `temp_dir / spec.repo`.
    Naming the root directory `spec.repo` ensures single-skill repositories with SKILL.md at the
    repository root keep their repository folder name for strict validation.
    """
    archive_path = temp_dir / "archive.zip"
    extract_dir = temp_dir / "extracted"
    repo_dest = temp_dir / "repo" / spec.repo
    repo_dest.parent.mkdir(parents=True, exist_ok=True)

    try:
        _download_commit_zip(spec, archive_path)
        extract_dir.mkdir(parents=True, exist_ok=True)
        _safe_extract_zip(archive_path, extract_dir)
        children = [p for p in extract_dir.iterdir() if p.is_dir()]
        source_root = children[0] if len(children) == 1 else extract_dir
        shutil.move(str(source_root), str(repo_dest))
    except GitHubDeployError:
        raise
    except (OSError, urllib.error.URLError, zipfile.BadZipFile, ValueError):
        if repo_dest.exists():
            shutil.rmtree(repo_dest)
        _git_fetch_commit_fallback(spec, repo_dest)

    return repo_dest


def discover_skill_dirs(repo_root: Path, subpath: str | None) -> list[Path]:
    resolved_root = repo_root.resolve()
    if subpath:
        scan_root = (resolved_root / Path(*PurePosixPath(subpath).parts)).resolve()
        if scan_root != resolved_root and resolved_root not in scan_root.parents:
            raise GitHubDeployError(
                "DEPLOY-GITHUB-003",
                f"Subpath '{subpath}' escapes the repository root.",
                "Pass a valid relative path inside the repository.",
                exit_code=2,
            )
        if not scan_root.exists():
            raise GitHubDeployError(
                "DEPLOY-GITHUB-003",
                f"Subpath '{subpath}' was not found in {resolved_root.name}.",
                "Check the folder path inside the repository or omit --subpath to list available skills.",
            )
        if scan_root.is_file():
            if scan_root.name == "SKILL.md":
                scan_root = scan_root.parent
            else:
                raise GitHubDeployError(
                    "DEPLOY-GITHUB-003",
                    f"Subpath '{subpath}' points to a file, not a skill directory.",
                    "Pass the directory that contains SKILL.md.",
                )
    else:
        scan_root = resolved_root

    found: list[Path] = []
    for current_root, dirnames, filenames in os.walk(scan_root, followlinks=False):
        dirnames[:] = sorted(
            d
            for d in dirnames
            if d not in IGNORED_WALK_DIRS and not (Path(current_root) / d).is_symlink()
        )
        if "SKILL.md" in filenames:
            skill_dir = Path(current_root).resolve()
            if not (skill_dir / "SKILL.md").is_symlink():
                sanitize_staged_skill_dir(skill_dir)
                found.append(skill_dir)

    found.sort(key=lambda p: p.relative_to(resolved_root).as_posix())
    return found


def relative_skill_subpath(repo_root: Path, skill_dir: Path) -> str:
    rel = skill_dir.resolve().relative_to(repo_root.resolve()).as_posix()
    return "" if rel == "." else rel


def scan_skill_scripts(skill_dir: Path) -> list[str]:
    """
    Return relative POSIX paths of all scripts or executable files inside `skill_dir`.
    """
    resolved_skill = skill_dir.resolve()
    detected: list[str] = []

    for current_root, dirnames, filenames in os.walk(resolved_skill, followlinks=False):
        dirnames[:] = sorted(d for d in dirnames if d not in IGNORED_WALK_DIRS)
        root_path = Path(current_root)
        rel_root_parts = root_path.relative_to(resolved_skill).parts

        for fname in sorted(filenames):
            if (
                fname in IGNORED_SKILL_INTERNAL_FILES
                or fname.lower() in STRIPPED_SKILL_ROOT_FILES_LOWER
                or fname.endswith((".pyc", ".pyo"))
            ):
                continue
            fpath = root_path / fname
            if fpath.is_symlink() or not fpath.is_file():
                continue

            rel_posix = fpath.relative_to(resolved_skill).as_posix()
            ext = fpath.suffix.lower()

            in_scripts_dir = len(rel_root_parts) > 0 and rel_root_parts[0] == "scripts"
            has_script_ext = ext in SCRIPT_OR_EXEC_EXTENSIONS

            has_shebang = False
            try:
                with fpath.open("rb") as fh:
                    prefix = fh.read(2)
                    has_shebang = prefix == b"#!"
            except OSError:
                pass

            is_exec_mode = False
            if os.name != "nt" and ext not in NON_EXECUTABLE_DOC_EXTENSIONS and fname != "SKILL.md":
                try:
                    is_exec_mode = bool(fpath.stat().st_mode & 0o111)
                except OSError:
                    pass

            if in_scripts_dir or has_script_ext or has_shebang or is_exec_mode:
                detected.append(rel_posix)

    return sorted(set(detected))


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def read_provenance(skill_dir: Path) -> dict[str, Any] | None:
    prov_path = skill_dir / PROVENANCE_FILE_NAME
    if not prov_path.is_file():
        return None
    try:
        data = json.loads(prov_path.read_text(encoding="utf-8"))
        return data if isinstance(data, dict) else None
    except (OSError, ValueError, json.JSONDecodeError):
        return None


def write_provenance(skill_dir: Path, payload: dict[str, Any]) -> Path:
    prov_path = skill_dir / PROVENANCE_FILE_NAME
    prov_path.write_text(json.dumps(payload, indent=2, sort_keys=False) + "\n", encoding="utf-8")
    return prov_path


def build_github_provenance(
    skill_name: str,
    spec: ResolvedGitHubSpec,
    skill_subpath: str,
    scripts: list[str],
    existing: dict[str, Any] | None = None,
) -> dict[str, Any]:
    now = utc_now_iso()
    installed_at = (
        existing.get("installedAt")
        if isinstance(existing, dict) and isinstance(existing.get("installedAt"), str)
        else now
    )
    return {
        "schemaVersion": 1,
        "skillName": skill_name,
        "sourceType": "github",
        "sourceUrl": spec.repo_url,
        "requestedUrl": spec.requested_url,
        "owner": spec.owner,
        "repo": spec.repo,
        "ref": spec.ref,
        "commit": spec.commit,
        "subpath": skill_subpath,
        "hasScripts": bool(scripts),
        "scripts": scripts,
        "installedAt": installed_at,
        "updatedAt": now,
    }


def build_local_provenance(
    skill_name: str,
    source_dir: Path,
    scripts: list[str],
    existing: dict[str, Any] | None = None,
) -> dict[str, Any]:
    now = utc_now_iso()
    installed_at = (
        existing.get("installedAt")
        if isinstance(existing, dict) and isinstance(existing.get("installedAt"), str)
        else now
    )
    return {
        "schemaVersion": 1,
        "skillName": skill_name,
        "sourceType": "local",
        "sourcePath": str(source_dir.resolve()),
        "hasScripts": bool(scripts),
        "scripts": scripts,
        "installedAt": installed_at,
        "updatedAt": now,
    }
