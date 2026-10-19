import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import todo


class TodoTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.file = str(Path(self.dir.name) / "todo.json")

    def tearDown(self):
        self.dir.cleanup()

    def run_cli(self, *args):
        out = io.StringIO()
        with redirect_stdout(out):
            code = todo.main(["--file", self.file, *args])
        return code, out.getvalue()

    def test_a_missing_file_is_an_empty_list(self):
        self.assertEqual(todo.load(self.file), [])

    def test_add_then_list(self):
        self.assertEqual(self.run_cli("add", "buy milk")[0], 0)
        self.assertEqual(self.run_cli("add", "call home")[0], 0)
        self.assertEqual(
            self.run_cli("list"), (0, "1. [ ] buy milk\n2. [ ] call home\n")
        )

    def test_done_marks_one_item(self):
        self.run_cli("add", "buy milk")
        self.run_cli("add", "call home")
        self.assertEqual(self.run_cli("done", "2")[0], 0)
        self.assertEqual(
            self.run_cli("list"), (0, "1. [ ] buy milk\n2. [x] call home\n")
        )

    def test_done_refuses_a_number_out_of_range(self):
        self.run_cli("add", "buy milk")
        self.assertEqual(self.run_cli("done", "2")[0], 1)
        self.assertEqual(self.run_cli("done", "0")[0], 1)

    def test_add_refuses_empty_text(self):
        with self.assertRaises(ValueError):
            todo.add([], "   ")


if __name__ == "__main__":
    unittest.main()
