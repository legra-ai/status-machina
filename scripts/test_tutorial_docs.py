"""The tutorial images must work as strict Rustdoc HTML, not only SVG files."""
import importlib.util
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("tutorial_renderer", ROOT / "scripts/render_tutorial_diagrams.py")
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Cannot load the tutorial diagram renderer")
RENDERER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RENDERER)


class TutorialDocumentationTests(unittest.TestCase):
    def test_every_shipped_svg_is_compatible_with_rustdoc_html_lints(self):
        for path in (ROOT / "docs/tutorials").glob("*.svg"):
            with self.subTest(path=path):
                svg = path.read_text()
                self.assertRegex(svg.splitlines()[0], r'^<svg .*?>$')
                self.assertNotRegex(svg, r'<(?:path|polygon|ellipse)\b[^>]*?/\s*>')
                self.assertIn('role="img"', svg)

    def test_graph_translation_rejects_unknown_syntax(self):
        with self.assertRaisesRegex(ValueError, "Unsupported canonical graph syntax"):
            RENDERER.graphviz('stateDiagram-v2\nclick s0 "https://example.org"\n')

    def test_graph_translation_preserves_escaped_labels_and_topology(self):
        dot = RENDERER.graphviz('stateDiagram-v2\n    state "A #38; B" as s0\n    [*] --> s0\n    s0 --> s0 : Again / Tick (internal)\n')
        self.assertIn('s0 [label="A & B"]', dot)
        self.assertIn('start -> s0;', dot)
        self.assertIn('s0 -> s0 [label="Again / Tick (internal)"]', dot)

    def test_crate_ships_the_rustdoc_images_and_executable_model_sources(self):
        include = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["include"]
        self.assertIn("docs/**/*.svg", include)
        self.assertIn("examples/**/*.rs", include)
        lib = (ROOT / "src/lib.rs").read_text()
        for name in ["order", "invoice"]:
            self.assertIn(f'include_str!("../docs/tutorials/{name}.svg")', lib)

    def test_gallery_regeneration_uses_the_standalone_repository_paths(self):
        model = (ROOT / "examples/diagrams/model.rs").read_text()
        self.assertIn('cargo run --quiet --example diagrams > docs/diagrams.md', model)
        self.assertNotIn('crates/status-machina/docs', model)


if __name__ == "__main__":
    unittest.main()
