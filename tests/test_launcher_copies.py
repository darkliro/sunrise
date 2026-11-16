"""The launcher's copies of game crates (launcher/crates/) stay byte- and value-equal to the game's originals."""
import ast
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COPIES = ROOT / "launcher/crates"

# (the game's file or folder, the launcher's copy)
SAME_BYTES = (
    ("crates/sunrise-release-sign/release-keys.txt", "release-verify/keys/sunrise-org-sunrise.txt"),
    ("crates/sunrise-release-sign/native-build-keys.txt", "release-verify/keys/native-builds.txt"),
    ("crates/sunrise-release-sign/tests/signed", "release-verify/tests/signed"),
    ("crates/sunrise-mod-package/tests/index", "smod/tests/index"),
    ("crates/sunrise-theme/tokens.css", "theme/tokens.css"),
)


def files(path: Path) -> dict:
    if path.is_file():
        return {"": path.read_bytes()}
    return {str(f.relative_to(path)): f.read_bytes() for f in sorted(path.rglob("*")) if f.is_file()}


def value(expression: str):
    """A Rust constant's value: a string literal, or integers joined by <<, *, + and -."""
    expression = expression.strip()
    if expression.startswith('"'):
        return ast.literal_eval(expression)
    try:
        tree = ast.parse(re.sub(r"(\d)_(?=\d)", r"\1", expression), mode="eval").body
    except SyntaxError as error:
        raise ValueError(f"not a constant this test reads: {expression}") from error

    def evaluate(node):
        if isinstance(node, ast.Constant) and isinstance(node.value, int):
            return node.value
        if isinstance(node, ast.BinOp):
            ops = {ast.LShift: lambda a, b: a << b, ast.Mult: lambda a, b: a * b,
                   ast.Add: lambda a, b: a + b, ast.Sub: lambda a, b: a - b}
            return ops[type(node.op)](evaluate(node.left), evaluate(node.right))
        raise ValueError(f"not a constant this test reads: {expression}")

    return evaluate(tree)


def constant(path: str, name: str):
    source = (ROOT / path).read_text(encoding="utf-8")
    found = re.search(rf"\bconst {name}\s*:\s*[^=]+=\s*(.+?);", source, re.S)
    if not found:
        raise AssertionError(f"{path} has no const {name}")
    return value(found.group(1))


class LauncherCopies(unittest.TestCase):
    def test_copied_files_are_the_games_byte_for_byte(self):
        for game, copy in SAME_BYTES:
            with self.subTest(copy=copy):
                original, copied = files(ROOT / game), files(COPIES / copy)
                self.assertTrue(original, f"{game} is empty or missing")
                self.assertEqual(sorted(copied), sorted(original), f"{copy} lists other files than {game}")
                for name in original:
                    where = f"{name} in " if name else ""
                    self.assertEqual(copied[name], original[name], f"{where}{copy} differs from {game}")

    def test_format_constants_match_the_games(self):
        package_format = int((ROOT / "crates/sunrise-mod-package/src/sdk/package-format").read_text().strip())
        self.assertEqual(constant("launcher/crates/smod/src/index/native.rs", "PACKAGE_FORMAT"), package_format)
        self.assertEqual(constant("launcher/crates/smod/src/manifest.rs", "USES_FILE"),
                         constant("crates/sunrise-mod-surface/src/uses.rs", "USES_FILE"))
        self.assertEqual(constant("launcher/crates/smod/src/manifest.rs", "MAX_USES"),
                         constant("crates/sunrise-mod-surface/src/uses.rs", "MAX_USES_BYTES"))

    def test_a_drifted_copy_is_caught(self):
        self.assertEqual(value("4 << 20"), 4 * 1024 * 1024)
        self.assertEqual(value("1_000"), 1000)
        self.assertEqual(value('"uses.json"'), "uses.json")
        with self.assertRaises(ValueError):
            value("mod_surface::uses::MAX_USES_BYTES as u64")


if __name__ == "__main__":
    unittest.main()
