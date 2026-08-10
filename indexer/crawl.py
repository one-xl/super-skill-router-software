#!/usr/bin/env python3
"""Build a static index of GitHub skills without downloading skill bodies."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
import time
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen

import yaml

API = "https://api.github.com"
CODE_SEARCH = f"{API}/search/code"
REPO_SEARCH = f"{API}/search/repositories"
RAW = "https://raw.githubusercontent.com"
USER_AGENT = "super-skill-router-indexer/0.2"


class CrawlError(RuntimeError):
    """An upstream response cannot produce a safe complete index record."""


@dataclass(frozen=True)
class SearchItem:
    repository: str
    default_branch: str | None
    path: str
    blob_sha: str
    discovery_source: str = "github-code-search"


@dataclass(frozen=True)
class Snapshot:
    default_branch: str
    commit_sha: str
    root_tree_sha: str
    repo_size_kb: int


def api_request(url: str, token: str, timeout: float, text: bool = False) -> Any:
    headers = {"Accept": "application/vnd.github+json", "User-Agent": USER_AGENT}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    for attempt in range(3):
        try:
            with urlopen(Request(url, headers=headers), timeout=timeout) as response:
                if text:
                    return response.read().decode("utf-8", errors="replace")
                payload = json.load(response)
                if not isinstance(payload, dict):
                    raise CrawlError(f"GitHub returned an invalid response for {url}")
                return payload
        except HTTPError as error:
            if error.code in {403, 429, 500, 502, 503, 504} and attempt < 2:
                retry_after = error.headers.get("Retry-After")
                time.sleep(min(float(retry_after), 30) if retry_after else 2**attempt)
                continue
            detail = error.read().decode("utf-8", errors="replace")
            raise CrawlError(f"GitHub request failed ({error.code}): {detail}") from error
        except (URLError, TimeoutError) as error:
            if attempt < 2:
                time.sleep(2**attempt)
                continue
            raise CrawlError(f"Could not reach GitHub: {error}") from error
    raise CrawlError(f"GitHub request failed after retries: {url}")


def parse_item(raw: Any, source: str = "github-code-search") -> SearchItem | None:
    if not isinstance(raw, dict) or not isinstance(raw.get("repository"), dict):
        return None
    repo = raw["repository"]
    values = (repo.get("full_name"), raw.get("path"), raw.get("sha"))
    if not all(isinstance(value, str) and value for value in values):
        return None
    branch = repo.get("default_branch") if isinstance(repo.get("default_branch"), str) else None
    return SearchItem(values[0], branch, values[1], values[2], source)


def search_code(query: str, max_results: int, token: str, timeout: float) -> tuple[list[SearchItem], int]:
    found: list[SearchItem] = []
    seen: set[tuple[str, str]] = set()
    page = 1
    total = 0
    while len(found) < max_results:
        params = urlencode({"q": query, "per_page": 100, "page": page})
        response = api_request(f"{CODE_SEARCH}?{params}", token, timeout)
        if page == 1:
            total = int(response.get("total_count", 0))
        items = response.get("items", [])
        if not isinstance(items, list) or not items:
            break
        for raw in items:
            item = parse_item(raw)
            if item is None or (item.repository, item.path) in seen:
                continue
            seen.add((item.repository, item.path))
            found.append(item)
            if len(found) >= max_results:
                break
        page += 1
    return found, total


def tree_entries(repository: str, tree_sha: str, token: str, timeout: float) -> list[dict[str, Any]]:
    repo = quote(repository, safe="/")
    tree = quote(tree_sha, safe="")
    payload = api_request(f"{API}/repos/{repo}/git/trees/{tree}", token, timeout)
    if payload.get("truncated"):
        raise CrawlError(f"{repository}: git tree {tree_sha} was truncated")
    entries = payload.get("tree")
    if not isinstance(entries, list) or not all(isinstance(entry, dict) for entry in entries):
        raise CrawlError(f"{repository}: git tree has an invalid entry list")
    return entries


def snapshot(item: SearchItem, token: str, timeout: float, cache: dict[str, Snapshot]) -> Snapshot:
    if item.repository in cache:
        return cache[item.repository]
    repo = quote(item.repository, safe="/")
    metadata = api_request(f"{API}/repos/{repo}", token, timeout)
    branch = item.default_branch or metadata.get("default_branch")
    size = metadata.get("size")
    if not isinstance(branch, str) or not branch:
        raise CrawlError(f"{item.repository}: default branch is unavailable")
    if not isinstance(size, int) or size < 0:
        raise CrawlError(f"{item.repository}: repository size is unavailable")
    commit = api_request(f"{API}/repos/{repo}/commits/{quote(branch, safe='')}", token, timeout)
    commit_sha = commit.get("sha")
    tree = commit.get("commit", {}).get("tree", {}) if isinstance(commit.get("commit"), dict) else {}
    root_tree_sha = tree.get("sha") if isinstance(tree, dict) else None
    if not isinstance(commit_sha, str) or not isinstance(root_tree_sha, str):
        raise CrawlError(f"{item.repository}: default branch did not resolve to a commit tree")
    value = Snapshot(branch, commit_sha, root_tree_sha, size)
    cache[item.repository] = value
    return value


def skill_directory(path: str) -> str:
    parent = path.rsplit("/", 1)[0] if "/" in path else "."
    return parent or "."


def skill_tree_sha(item: SearchItem, snap: Snapshot, token: str, timeout: float) -> tuple[str, str]:
    directory = skill_directory(item.path)
    if directory == ".":
        return directory, snap.root_tree_sha
    current = snap.root_tree_sha
    for segment in directory.split("/"):
        entry = next((entry for entry in tree_entries(item.repository, current, token, timeout) if entry.get("path") == segment and entry.get("type") == "tree"), None)
        current = entry.get("sha") if isinstance(entry, dict) else None
        if not isinstance(current, str) or not current:
            raise CrawlError(f"{item.repository}: skill directory '{directory}' is missing")
    return directory, current


def complete_files(repository: str, tree_sha: str, token: str, timeout: float) -> list[dict[str, int | str]]:
    repo = quote(repository, safe="/")
    tree = quote(tree_sha, safe="")
    payload = api_request(f"{API}/repos/{repo}/git/trees/{tree}?recursive=1", token, timeout)
    if payload.get("truncated"):
        raise CrawlError(f"{repository}: recursive tree was truncated; complete manifest unavailable")
    entries = payload.get("tree")
    if not isinstance(entries, list):
        raise CrawlError(f"{repository}: recursive tree has no file list")
    files: list[dict[str, int | str]] = []
    for entry in entries:
        if entry.get("type") == "tree":
            continue
        if entry.get("type") != "blob" or not isinstance(entry.get("path"), str) or not isinstance(entry.get("size"), int):
            raise CrawlError(f"{repository}: unsupported or malformed entry in recursive tree")
        files.append({"path": entry["path"], "size": entry["size"]})
    files.sort(key=lambda file: str(file["path"]).casefold())
    if not any(file["path"] == "SKILL.md" for file in files):
        raise CrawlError(f"{repository}: complete manifest has no SKILL.md")
    return files


def raw_url(repository: str, commit_sha: str, path: str) -> str:
    return f"{RAW}/{quote(repository, safe='/')}/{quote(commit_sha, safe='')}/{quote(path, safe='/')}"


def frontmatter(document: str) -> dict[str, Any]:
    lines = document.splitlines()
    if not lines or lines[0].strip() != "---":
        raise CrawlError("SKILL.md is missing YAML frontmatter")
    try:
        end = next(index for index, line in enumerate(lines[1:], 1) if line.strip() == "---")
    except StopIteration as error:
        raise CrawlError("SKILL.md frontmatter is not terminated") from error
    parsed = yaml.safe_load("\n".join(lines[1:end]))
    if not isinstance(parsed, dict):
        raise CrawlError("SKILL.md frontmatter is not a mapping")
    return parsed


def text_value(value: Any) -> str:
    return value.strip() if isinstance(value, str) else ""


def tags_value(value: Any) -> list[str]:
    values = value if isinstance(value, list) else value.split(",") if isinstance(value, str) else []
    return sorted({tag.strip() for tag in values if isinstance(tag, str) and tag.strip()}, key=str.casefold)


def build_index(items: Iterable[SearchItem], token: str, timeout: float) -> tuple[list[dict[str, Any]], list[str]]:
    records: list[dict[str, Any]] = []
    warnings: list[str] = []
    cache: dict[str, Snapshot] = {}
    for item in items:
        try:
            print(f"Indexing {item.repository}/{item.path}", file=sys.stderr, flush=True)
            snap = snapshot(item, token, timeout, cache)
            directory, directory_tree = skill_tree_sha(item, snap, token, timeout)
            files = complete_files(item.repository, directory_tree, token, timeout)
            metadata = frontmatter(api_request(raw_url(item.repository, snap.commit_sha, item.path), token, timeout, text=True))
            name, description = text_value(metadata.get("name")), text_value(metadata.get("description"))
            if not name or not description:
                raise CrawlError("SKILL.md frontmatter requires name and description")
            records.append({
                "id": hashlib.sha256(f"{item.repository}:{directory}".encode()).hexdigest()[:20],
                "name": name,
                "description": description,
                "whenToUse": text_value(metadata.get("whenToUse", metadata.get("when_to_use"))),
                "tags": tags_value(metadata.get("tags")),
                "repo": item.repository,
                "path": directory,
                "default_branch": snap.default_branch,
                "commit_sha": snap.commit_sha,
                "files": files,
                "repo_size_kb": snap.repo_size_kb,
                "source": {"repository": item.repository, "skillFilePath": item.path, "ref": snap.default_branch, "blobSha": item.blob_sha, "rawUrl": raw_url(item.repository, snap.commit_sha, item.path), "discoverySource": item.discovery_source},
            })
        except (CrawlError, HTTPError) as error:
            warnings.append(f"{item.repository}/{item.path}: {error}")
    records.sort(key=lambda record: (record["name"].casefold(), record["repo"], record["path"]))
    return records, warnings


def write_atomic(output: Path, payload: dict[str, Any]) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=output.parent, delete=False) as handle:
        handle.write(json.dumps(payload, ensure_ascii=False, indent=2) + "\n")
        temporary = Path(handle.name)
    try:
        os.replace(temporary, output)
    except OSError:
        temporary.unlink(missing_ok=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--query", default="filename:SKILL.md")
    parser.add_argument("--max-results", type=int, default=1000)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--output", type=Path, default=Path(__file__).with_name("index.json"))
    parser.add_argument("--token", default=os.environ.get("GITHUB_TOKEN", ""))
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be greater than zero")
    try:
        items, total = search_code(args.query, args.max_results, args.token, args.timeout)
        records, warnings = build_index(items, args.token, args.timeout)
        write_atomic(args.output, {"schemaVersion": 1, "status": "complete", "generatedAt": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"), "query": args.query, "sourceMatches": total, "processedMatches": len(items), "indexedSkills": len(records), "truncated": total > len(items), "skills": records, "warnings": warnings})
        print(f"Indexed {len(records)} skills from {len(items)} matches: {args.output}")
        return 0
    except (CrawlError, OSError) as error:
        print(f"Index build failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
