import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

MODULE = Path(__file__).parents[1] / "crawl.py"
SPEC = importlib.util.spec_from_file_location("crawl", MODULE)
assert SPEC and SPEC.loader
crawl = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = crawl
SPEC.loader.exec_module(crawl)


class CrawlTests(unittest.TestCase):
    def item(self, branch: str | None = None) -> crawl.SearchItem:
        return crawl.SearchItem("example/skills", branch, "skills/pdf/SKILL.md", "blob-sha")

    def test_code_search_result_without_branch_is_retained(self) -> None:
        item = crawl.parse_item({"repository": {"full_name": "example/skills"}, "path": "SKILL.md", "sha": "sha"})
        self.assertIsNotNone(item)
        assert item
        self.assertIsNone(item.default_branch)

    def test_complete_manifest_contains_nested_files(self) -> None:
        def fake(url: str, token: str, timeout: float, text: bool = False):
            if url.endswith("/git/trees/tree?recursive=1"):
                return {"truncated": False, "tree": [{"path": "SKILL.md", "type": "blob", "size": 20}, {"path": "scripts/run.py", "type": "blob", "size": 10}]}
            self.fail(url)

        with patch.object(crawl, "api_request", side_effect=fake):
            self.assertEqual(crawl.complete_files("example/skills", "tree", "token", 1), [{"path": "scripts/run.py", "size": 10}, {"path": "SKILL.md", "size": 20}])

    def test_truncated_manifest_is_rejected(self) -> None:
        with patch.object(crawl, "api_request", return_value={"truncated": True}):
            with self.assertRaises(crawl.CrawlError):
                crawl.complete_files("example/skills", "tree", "token", 1)

    def test_frontmatter_and_atomic_write(self) -> None:
        self.assertEqual(crawl.frontmatter("---\nname: PDF\ndescription: Work.\n---\nbody")['name'], "PDF")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "index.json"
            crawl.write_atomic(output, {"skills": []})
            self.assertEqual(json.loads(output.read_text(encoding="utf-8")), {"skills": []})


if __name__ == "__main__":
    unittest.main()
