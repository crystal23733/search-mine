import importlib.util
from pathlib import Path
import tempfile
import unittest
spec = importlib.util.spec_from_file_location("build_web", Path(__file__).resolve().parents[1] / "build_web.py")
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)

class BuildWebTests(unittest.TestCase):
    def test_policy_content_changes_the_build_cache_key_and_stays_out_of_the_context(self):
        with tempfile.TemporaryDirectory() as folder:
            policy = Path(folder) / "public site.json"
            policy.write_text('{"revision":"one"}')
            first = build.command("https://example.test", "fixture:web", policy, "ca-pub-0000000000000001")
            policy.write_text('{"revision":"two"}')
            second = build.command("https://example.test", "fixture:web", policy, "ca-pub-0000000000000001")
            self.assertNotEqual(first, second)
            self.assertEqual(first[-1], ".")
            self.assertIn("id=public_site_config,src=" + str(policy.resolve()), first)
            self.assertNotIn('{"revision":"one"}', str(first))
        with self.assertRaises(ValueError):
            build.command("https://example.test", "fixture:web", publisher="ca-pub-0000000000000001")
