"""Synthetic archive checks only; never invoke Cargo or use a build cache."""

from contextlib import redirect_stderr, redirect_stdout
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import check_package_archive as checker
from check_package import PolicyError


ORIGINAL = b'''[package]
name = "ideomancer"
version = "0.1.0"
edition = "2021"
rust-version = "1.89"
license = "MIT"
[lib]
name = "ideomancer_core"
'''
NORMALIZED_FLAGS = b'''build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
'''
NORMALIZED = (checker.NORMALIZED_HEADER + ORIGINAL.replace(b'[lib]\n', NORMALIZED_FLAGS + b'[lib]\n')
              + b'path = "src/lib.rs"\n'
              b'[[bin]]\nname = "ideomancer"\npath = "src/main.rs"\n')
VCS = {"git": {"sha1": "a" * 40, "dirty": True}, "path_in_vcs": ""}


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.archive = self.root / "candidate.crate"
        self.contents = {"Cargo.toml": NORMALIZED, "Cargo.toml.orig": ORIGINAL,
                         "src/lib.rs": b"// synthetic\n", "src/main.rs": b"fn main() {}\n",
                         "Cargo.lock": b"# exact lock\n",
                         ".github/workflows/rust.yml": b"name: synthetic\n"}
        self.expected = set(self.contents)

    def write_archive(self, contents=None, extra=(), raw_tail=b"", truncate=0):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w", format=tarfile.USTAR_FORMAT) as output:
            for name, content in (self.contents if contents is None else contents).items():
                entry = tarfile.TarInfo(checker.ROOT + "/" + name)
                entry.size = len(content)
                output.addfile(entry, io.BytesIO(content))
            for entry, data in extra:
                output.addfile(entry, io.BytesIO(data))
        compressed = gzip.compress(buffer.getvalue() + raw_tail)
        self.archive.write_bytes(compressed[:-truncate] if truncate else compressed)

    def tree(self, name, source=False):
        root = self.root / name
        root.mkdir()
        for relative, content in self.contents.items():
            if source and relative in {"Cargo.toml", ".cargo_vcs_info.json"}:
                continue
            if source and relative == "Cargo.toml.orig":
                relative = "Cargo.toml"
            file = root / relative
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(content)
        return root

    def verify(self, **kwargs):
        return checker.verify_archive(self.archive, self.expected, **kwargs)

    def test_success_receipt_source_extracted_and_optional_vcs(self):
        self.contents[".cargo_vcs_info.json"] = json.dumps(VCS).encode()
        self.write_archive()
        source = self.tree("source", source=True)
        extracted = self.tree("extracted")
        (extracted / "target").mkdir()
        (extracted / "target/unreviewed-output").write_bytes(b"allowed extra")
        receipt = self.verify(source_dir=source, extracted_dir=extracted)
        self.assertEqual(receipt["status"], "passed")
        self.assertEqual(receipt["archive_sha256"], hashlib.sha256(self.archive.read_bytes()).hexdigest())
        self.assertEqual(receipt["file_count"], 7)
        self.assertEqual(receipt["vcs"], VCS["git"])
        self.assertEqual(receipt["package"]["name"], "ideomancer")
        self.assertTrue(receipt["matched_source"])
        self.assertTrue(receipt["matched_extracted"])
        self.assertNotIn(str(self.root), json.dumps(receipt))

    def test_comparisons_are_explicit(self):
        self.write_archive()
        receipt = self.verify()
        self.assertFalse(receipt["matched_source"])
        self.assertFalse(receipt["matched_extracted"])
        self.assertIsNone(receipt["vcs"])

    def test_clean_vcs_omitted_or_false_dirty_normalized_in_receipt(self):
        for git in ({"sha1": "a" * 40}, {"sha1": "a" * 40, "dirty": False}):
            self.write_archive(contents={**self.contents, ".cargo_vcs_info.json":
                                         json.dumps({**VCS, "git": git}).encode()})
            self.assertEqual(self.verify()["vcs"], {"sha1": "a" * 40, "dirty": False})

    def test_dependency_normalization_preserves_defaults_and_feature_sets(self):
        original = ORIGINAL + b'''
[dependencies]
plain = "1"
custom = { version = "2", features = ["a", "b"], optional = true, default-features = false, package = "renamed" }
[build-dependencies]
builder = "3"
[dev-dependencies]
tester = "4"
[target.'cfg(unix)'.dependencies]
unix = "5"
[features]
default = ["dep:custom", "plain/std"]
'''
        normalized = NORMALIZED + b'''
[dependencies]
plain = { version = "1", default-features = true, optional = false, features = [], package = "plain" }
custom = { version = "2", features = ["b", "a"], optional = true, default-features = false, package = "renamed" }
[build-dependencies.builder]
version = "3"
[dev-dependencies.tester]
version = "4"
[target.'cfg(unix)'.dependencies.unix]
version = "5"
[features]
default = ["plain/std", "dep:custom"]
'''
        self.contents.update({"Cargo.toml": normalized, "Cargo.toml.orig": original})
        self.write_archive()
        source = self.tree("source", source=True)
        self.assertTrue(self.verify(source_dir=source)["matched_source"])

    def test_registry_dependency_mutations_fail_source_match(self):
        requests = b'\n[dependencies]\nbase = {version="1", features=["std"]}\n'
        self.contents.update({"Cargo.toml": NORMALIZED + requests,
                              "Cargo.toml.orig": ORIGINAL + requests})
        source = self.tree("source", source=True)
        for changed in (requests.replace(b'version="1"', b'version="2"'),
                        requests.replace(b'"std"', b'"extra"'),
                        requests.replace(b'features=', b'default-features=false, features='),
                        requests.replace(b'features=', b'optional=true, features='),
                        requests.replace(b'features=', b'package="renamed", features='),
                        requests.replace(b'features=', b'registry="alternate", features='),
                        requests.replace(b'features=', b'registry-index="https://example.invalid", features='),
                        requests + b'added = "1"\n', b''):
            with self.subTest(changed=changed):
                self.write_archive(contents={**self.contents, "Cargo.toml": NORMALIZED + changed})
                with self.assertRaises(PolicyError):
                    self.verify(source_dir=source)

    def test_all_dependency_kinds_and_target_requests_compared(self):
        for table in (b'dependencies', b'build-dependencies', b'dev-dependencies',
                      b'target."cfg(unix)".dependencies',
                      b'target."cfg(unix)".build-dependencies',
                      b'target."cfg(unix)".dev-dependencies'):
            requests = b'\n[' + table + b']\nbase="1"\n'
            self.write_archive(contents={**self.contents, "Cargo.toml.orig": ORIGINAL + requests,
                                         "Cargo.toml": NORMALIZED + requests.replace(b'"1"', b'"2"')})
            with self.subTest(table=table), self.assertRaises(PolicyError):
                self.verify()

    def test_feature_table_changes_rejected(self):
        for original, normalized in ((b'', b'\n[features]\ndefault=["other"]\n'),
                                     (b'\n[features]\ndefault=["base"]\n',
                                      b'\n[features]\ndefault=["other"]\n')):
            self.write_archive(contents={**self.contents, "Cargo.toml.orig": ORIGINAL + original,
                                         "Cargo.toml": NORMALIZED + normalized})
            with self.assertRaises(PolicyError):
                self.verify()

    def test_build_hooks_bin_paths_and_library_overrides_rejected(self):
        mutations = [NORMALIZED.replace(b'build = false', b'build="src/main.rs"'),
                     NORMALIZED.replace(b'version = "0.1.0"',
                                        b'version = "0.1.0"\nlinks="native-hook"'),
                     NORMALIZED.replace(b'src/main.rs', b'src/lib.rs'),
                     NORMALIZED.replace(b'name = "ideomancer"\npath', b'name = "other"\npath'),
                     NORMALIZED + b'[[bin]]\nname="extra"\npath="src/main.rs"\n',
                     NORMALIZED.split(b'[[bin]]')[0]]
        for field, value in ((b'proc-macro', b'true'), (b'doctest', b'false'),
                             (b'test', b'false'), (b'bench', b'false'), (b'doc', b'false'),
                             (b'harness', b'false'), (b'crate-type', b'["cdylib"]'),
                             (b'edition', b'"2018"'), (b'test', b'1')):
            mutations.append(NORMALIZED.replace(b'[[bin]]', field + b'=' + value + b'\n[[bin]]'))
        for normalized in mutations:
            with self.subTest(normalized=normalized):
                self.write_archive(contents={**self.contents, "Cargo.toml": normalized})
                with self.assertRaises(PolicyError):
                    self.verify()

    def test_generated_package_defaults_and_library_defaults_accepted(self):
        normalized = NORMALIZED.replace(b'edition = "2021"', b'edition = "2021"\nreadme = "README.md"')
        normalized = normalized.replace(b'[[bin]]', b'proc-macro=false\ntest=true\ndoctest=true\n[[bin]]')
        self.write_archive(contents={**self.contents, "Cargo.toml": normalized})
        self.assertEqual(self.verify()["status"], "passed")

    def test_all_normalized_build_and_discovery_flags_required(self):
        self.write_archive(contents={**self.contents, "Cargo.toml": NORMALIZED.replace(NORMALIZED_FLAGS, b"")})
        with self.assertRaises(PolicyError):
            self.verify()

    def test_each_normalized_build_and_discovery_flag_required_false(self):
        for line in NORMALIZED_FLAGS.splitlines(keepends=True):
            for replacement in (b"", line.replace(b"false", b"true"),
                                line.replace(b"false", b"0"), line.replace(b"false", b'"false"'),
                                line.replace(b"false", b"[]"), line.replace(b"false", b"{}")):
                with self.subTest(flag=line, replacement=replacement):
                    self.write_archive(contents={**self.contents, "Cargo.toml": NORMALIZED.replace(line, replacement)})
                    with self.assertRaises(PolicyError):
                        self.verify()

    def test_extracted_extra_sources_configs_and_empty_directories_rejected(self):
        self.write_archive()
        for index, extra in enumerate(("build.rs", ".cargo/config.toml", "src/unknown.rs", "empty/")):
            extracted = self.tree(f"extracted-{index}")
            path = extracted / extra
            if extra.endswith("/"):
                path.mkdir()
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"// unarchived source or configuration\n")
            with self.subTest(extra=extra), self.assertRaises(PolicyError):
                self.verify(extracted_dir=extracted)

    def test_extracted_unarchived_symlinks_are_not_whitelisted(self):
        self.write_archive()
        outside = self.root / "outside"
        outside.mkdir()
        for index, extra in enumerate(("target", "unknown", ".cargo", "src/unknown")):
            extracted = self.tree(f"extracted-{index}")
            (extracted / extra).symlink_to(outside, target_is_directory=True)
            with self.subTest(extra=extra), self.assertRaises(PolicyError):
                self.verify(extracted_dir=extracted)

    def test_extracted_target_must_be_directory(self):
        self.write_archive()
        extracted = self.tree("extracted")
        (extracted / "target").write_bytes(b"not build output directory")
        with self.assertRaises(PolicyError):
            self.verify(extracted_dir=extracted)

    def test_extracted_scan_is_bounded_and_skips_real_target_subtree(self):
        self.write_archive()
        extracted = self.tree("extracted")
        with patch.object(checker, "MAX_TREE_ENTRIES", 2):
            with self.assertRaisesRegex(PolicyError, "entry count"):
                self.verify(extracted_dir=extracted)
        (extracted / "target/deep/build/output").mkdir(parents=True)
        (extracted / "target/deep/build/output/artifact").write_bytes(b"extra build output")
        # No access is needed inside target, including following build-cache links.
        (extracted / "target/unused-link").symlink_to(self.root / "missing")
        with patch.object(checker, "MAX_TREE_ENTRIES", 12):
            self.assertTrue(self.verify(extracted_dir=extracted)["matched_extracted"])

    def test_extracted_directory_changes_during_scan_rejected(self):
        self.write_archive()
        extracted = self.tree("extracted")
        original_scandir = checker.os.scandir
        changed = False

        def change(directory):
            nonlocal changed
            if not changed:
                changed = True
                temporary = extracted / "transient-extra"
                temporary.mkdir()
                temporary.rmdir()
            return original_scandir(directory)

        with patch.object(checker.os, "scandir", side_effect=change):
            with self.assertRaisesRegex(PolicyError, "directory changed"):
                self.verify(extracted_dir=extracted)

    def test_test_targets_are_only_conventional_reviewed_rust_tests(self):
        self.contents["tests/synthetic.rs"] = b"// test\n"
        self.expected.add("tests/synthetic.rs")
        declaration = b'\n[[test]]\nname="synthetic"\npath="tests/synthetic.rs"\n'
        self.write_archive(contents={**self.contents, "Cargo.toml": NORMALIZED + declaration})
        self.assertEqual(self.verify()["status"], "passed")
        for changed in (b'', declaration.replace(b'tests/synthetic.rs', b'src/main.rs'),
                        declaration + declaration, declaration + b'harness=false\n'):
            self.write_archive(contents={**self.contents, "Cargo.toml": NORMALIZED + changed})
            with self.assertRaises(PolicyError):
                self.verify()

    def test_malformed_dependency_and_target_shapes_are_policy_errors(self):
        for fragment in (b'target=1\n', b'target=[]\n', b'[target]\n"cfg(unix)"=1\n',
                         b'[target."cfg(unix)"]\ndependencies=[]\n',
                         b'[target."cfg(unix)"]\nlib={}\n',
                         b'[dependencies]\nbase={version="1",features=1}\n',
                         b'[dependencies]\nbase={version="1",optional=1}\n',
                         b'[dependencies]\nbase={version="1",default-features=1}\n',
                         b'[features]\ndefault=1\n'):
            # Root scalar keys must precede all table headers.
            normalized = checker.NORMALIZED_HEADER + fragment + NORMALIZED[len(checker.NORMALIZED_HEADER):]
            self.write_archive(contents={**self.contents, "Cargo.toml": normalized})
            with self.subTest(fragment=fragment), self.assertRaises(PolicyError):
                self.verify()

    def test_workspace_and_patch_rejected_in_original_too(self):
        for fragment in (b'\n[workspace]\n', b'\n[patch.crates-io]\n', b'\n[replace]\n'):
            self.write_archive(contents={**self.contents, "Cargo.toml.orig": ORIGINAL + fragment})
            with self.assertRaises(PolicyError):
                self.verify()

    def test_content_mismatch_and_length_mismatch(self):
        self.write_archive()
        for source in (True, False):
            root = self.tree("source" if source else "extracted", source=source)
            for content in (b"wrong", self.contents["src/lib.rs"] + b"extra", b""):
                with self.subTest(source=source, content=content):
                    (root / "src/lib.rs").write_bytes(content)
                    with self.assertRaises(PolicyError):
                        self.verify(**{"source_dir" if source else "extracted_dir": root})

    def test_lockfile_compared_exactly(self):
        self.write_archive()
        root = self.tree("source", source=True)
        (root / "Cargo.lock").write_bytes(b"different normalized lock")
        with self.assertRaises(PolicyError):
            self.verify(source_dir=root)

    def test_source_original_manifest_compared(self):
        self.write_archive()
        root = self.tree("source", source=True)
        (root / "Cargo.toml").write_bytes(NORMALIZED)
        with self.assertRaises(PolicyError):
            self.verify(source_dir=root)

    def test_extracted_normalized_manifest_compared(self):
        self.write_archive()
        root = self.tree("extracted")
        (root / "Cargo.toml").write_bytes(ORIGINAL)
        with self.assertRaises(PolicyError):
            self.verify(extracted_dir=root)

    def test_bad_member_names(self):
        for name in ("../escape", "/absolute", "wrong-0.1.0/file", checker.ROOT + "/src/../escape",
                     checker.ROOT + "/src//lib.rs", checker.ROOT + "/./file",
                     checker.ROOT + "/src\\evil", checker.ROOT + "/unreviewed",
                     checker.ROOT + "/target/file"):
            with self.subTest(name=name):
                self.write_archive(extra=[(tarfile.TarInfo(name), b"")])
                with self.assertRaises(PolicyError):
                    self.verify()

    def test_duplicate_file(self):
        self.write_archive(extra=[(tarfile.TarInfo(checker.ROOT + "/src/lib.rs"), b"")])
        with self.assertRaisesRegex(PolicyError, "duplicate"):
            self.verify()

    def test_missing_member(self):
        self.write_archive(contents={k: v for k, v in self.contents.items() if k != "Cargo.lock"})
        with self.assertRaisesRegex(PolicyError, "missing"):
            self.verify()

    def test_links_special_files_and_extensions(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE,
                     tarfile.XHDTYPE, tarfile.GNUTYPE_LONGNAME, tarfile.GNUTYPE_SPARSE):
            with self.subTest(kind=kind):
                entry = tarfile.TarInfo(checker.ROOT + "/link")
                entry.type = kind
                entry.linkname = "src/lib.rs"
                self.write_archive(extra=[(entry, b"")])
                with self.assertRaises(PolicyError):
                    self.verify()

    def test_canonical_directories_including_reviewed_hidden_parent(self):
        entries = []
        for name in (checker.ROOT + "/", checker.ROOT + "/src/", checker.ROOT + "/.github/",
                     checker.ROOT + "/.github/workflows/"):
            entry = tarfile.TarInfo(name)
            entry.type = tarfile.DIRTYPE
            entries.append((entry, b""))
        self.write_archive(extra=entries)
        self.assertEqual(self.verify()["status"], "passed")

    def test_unreviewed_and_noncanonical_directories(self):
        for suffix in ("unreviewed/", "src//", "src/../src/", "src/./"):
            with self.subTest(suffix=suffix):
                entry = tarfile.TarInfo(checker.ROOT + "/" + suffix)
                entry.type = tarfile.DIRTYPE
                self.write_archive(extra=[(entry, b"")])
                with self.assertRaises(PolicyError):
                    self.verify()

    def test_invalid_manifests(self):
        bad = [ORIGINAL, NORMALIZED.replace(b'0.1.0', b'0.2.0'),
               NORMALIZED.replace(b'1.89', b'1.90'), NORMALIZED.replace(b'MIT', b'other'),
               NORMALIZED.replace(b'ideomancer_core', b'other'),
               NORMALIZED.replace(b'src/lib.rs', b'../src/lib.rs'),
               NORMALIZED + b'\n[workspace]\n', NORMALIZED + b'\n[patch]\n',
               checker.NORMALIZED_HEADER + b'SECRET INVALID TOML',
               NORMALIZED + b'\n[target."cfg(unix)".dependencies.bad]\npath=".."\n',
               NORMALIZED + b'\n[dependencies.bad]\ngit="https://example.invalid"\n',
               NORMALIZED + b'\n[build-dependencies.bad]\nworkspace=true\n',
               NORMALIZED + b'\n[dependencies]\nbad=123\n',
               checker.NORMALIZED_HEADER + b'target = 12\n' + ORIGINAL + b'path="src/lib.rs"\n']
        for content in bad:
            with self.subTest(content=content):
                self.write_archive(contents={**self.contents, "Cargo.toml": content})
                with self.assertRaises(PolicyError):
                    self.verify()

    def test_invalid_original_identity(self):
        self.write_archive(contents={**self.contents, "Cargo.toml.orig": ORIGINAL.replace(b"0.1.0", b"0.2.0")})
        with self.assertRaises(PolicyError):
            self.verify()

    def test_duplicate_vcs_keys_and_excessive_json_nesting(self):
        for content in (b'{"git":{},"git":{},"path_in_vcs":""}',
                        b'[' * 2000 + b']' * 2000):
            self.write_archive(contents={**self.contents, ".cargo_vcs_info.json": content})
            with self.assertRaises(PolicyError):
                self.verify()

    def test_invalid_vcs_receipts(self):
        for value in ({}, [], {**VCS, "path_in_vcs": "elsewhere"},
                      {**VCS, "extra": True}, {**VCS, "git": {"sha1": "a", "dirty": True}},
                      {**VCS, "git": {"sha1": "a" * 40, "dirty": 1}},
                      {**VCS, "git": {"sha1": "z" * 40, "dirty": False}}):
            with self.subTest(value=value):
                self.write_archive(contents={**self.contents, ".cargo_vcs_info.json": json.dumps(value).encode()})
                with self.assertRaises(PolicyError):
                    self.verify()
        self.write_archive(contents={**self.contents, ".cargo_vcs_info.json": b"not json"})
        with self.assertRaises(PolicyError):
            self.verify()

    def test_corrupt_gzip_and_tar(self):
        self.write_archive()
        valid = self.archive.read_bytes()
        for content in (b"not gzip", valid[:-1], valid[:-8], valid[:25],
                        valid[:-8] + bytes(8), gzip.compress(b"short"),
                        gzip.compress(b"x" * 1024)):
            with self.subTest(length=len(content)):
                self.archive.write_bytes(content)
                with self.assertRaises((PolicyError, OSError)):
                    self.verify()

    def test_truncated_tar_after_valid_header(self):
        self.write_archive()
        raw = gzip.decompress(self.archive.read_bytes())
        for length in (512, 1024, 2048):
            self.archive.write_bytes(gzip.compress(raw[:length]))
            with self.assertRaises(PolicyError):
                self.verify()

    def test_nonzero_trailing_data(self):
        self.write_archive(raw_tail=b"secret trailing payload")
        with self.assertRaises(PolicyError):
            self.verify()

    def test_all_limits(self):
        self.write_archive()
        for name, value in (("MAX_COMPRESSED", 10), ("MAX_FILE", 10),
                            ("MAX_TOTAL", 10), ("MAX_MEMBERS", 2)):
            with self.subTest(limit=name), patch.object(checker, name, value):
                with self.assertRaises(PolicyError):
                    self.verify()

    def test_huge_advertised_size_rejected_without_payload(self):
        for size in (checker.MAX_FILE + 1, 2 ** 40, -1):
            entry = tarfile.TarInfo(checker.ROOT + "/src/lib.rs")
            entry.size = size
            self.archive.write_bytes(gzip.compress(entry.tobuf(format=tarfile.GNU_FORMAT) + bytes(1024)))
            with self.assertRaises(PolicyError):
                self.verify()

    def test_zero_length_file_allowed(self):
        self.write_archive(contents={**self.contents, "src/lib.rs": b""})
        self.assertEqual(self.verify()["status"], "passed")

    def test_bounded_zero_trailer(self):
        self.write_archive(raw_tail=bytes(2 * 1024 * 1024))
        with patch.object(checker, "MAX_TOTAL", 1024):
            with self.assertRaisesRegex(PolicyError, "trailing"):
                self.verify()

    def test_symlink_root_component_and_final_file(self):
        self.write_archive()
        for source in (True, False):
            key = "source_dir" if source else "extracted_dir"
            root = self.tree(key, source=source)
            alias = self.root / (key + "-alias")
            alias.symlink_to(root, target_is_directory=True)
            with self.subTest(key=key, location="root"), self.assertRaises(OSError):
                self.verify(**{key: alias})
            (root / "src").rename(root / "real-src")
            (root / "src").symlink_to(root / "real-src", target_is_directory=True)
            with self.subTest(key=key, location="component"), self.assertRaises(OSError):
                self.verify(**{key: root})
            (root / "src").unlink()
            (root / "real-src").rename(root / "src")
            file = root / "src/lib.rs"
            file.rename(root / "saved.rs")
            file.symlink_to(root / "saved.rs")
            with self.subTest(key=key, location="file"), self.assertRaises(OSError):
                self.verify(**{key: root})

    def test_file_changed_during_comparison(self):
        root = self.tree("source", source=True)
        with self.assertRaisesRegex(PolicyError, "changed"):
            with checker.checked_file(root, "src/lib.rs") as stream:
                stream.read()
                (root / "src/lib.rs").write_bytes(b"new bytes")

    def test_archive_changed_during_inspection(self):
        self.write_archive()
        # Change while gzip is being consumed, before the final archive fstat.
        original_frombuf = tarfile.TarInfo.frombuf
        changed = False

        def change(*args, **kwargs):
            nonlocal changed
            if not changed:
                changed = True
                with self.archive.open("ab") as output:
                    output.write(b"changed")
            return original_frombuf(*args, **kwargs)

        with patch.object(tarfile.TarInfo, "frombuf", side_effect=change):
            with self.assertRaises((PolicyError, OSError)):
                self.verify()

    def test_cli_failure_is_json_and_does_not_leak_contents_or_paths(self):
        self.archive.write_bytes(b"SECRET INVALID ARCHIVE")
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            code = checker.main(["--archive", str(self.archive)])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(stdout.getvalue())["status"], "failed")
        self.assertNotIn("SECRET", stdout.getvalue() + stderr.getvalue())
        self.assertNotIn(str(self.root), stdout.getvalue() + stderr.getvalue())

    def test_cli_uses_trusted_inventory_not_archive_inventory(self):
        self.write_archive()
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            code = checker.main(["--archive", str(self.archive)])
        self.assertEqual(code, 1)  # Synthetic inventory cannot approve itself.
        self.assertEqual(json.loads(stdout.getvalue())["status"], "failed")

    def test_cli_success_and_manifest_error_redaction(self):
        for normalized, expected_code in ((NORMALIZED, 0),
                                          (checker.NORMALIZED_HEADER + b"SECRET TOML", 1)):
            self.write_archive(contents={**self.contents, "Cargo.toml": normalized})
            stdout, stderr = io.StringIO(), io.StringIO()
            with patch.object(checker, "inventory", return_value=self.expected):
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    code = checker.main(["--archive", str(self.archive)])
            self.assertEqual(code, expected_code)
            receipt = json.loads(stdout.getvalue())
            self.assertEqual(receipt["status"], "passed" if code == 0 else "failed")
            self.assertNotIn("SECRET", stdout.getvalue() + stderr.getvalue())
            self.assertNotIn(str(self.root), stdout.getvalue() + stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
