# Cached fixtures for Python test suites

`gix.testtools` binds `gix-testtools`' native fixture cache. It is available in
every build, including reduced hash configurations. Paths and return values
follow the Rust API; fixture paths are `pathlib.Path` objects.

Run tests from the test project's root. Fixtures and scripts live under
`tests/fixtures`, generated directories under
`tests/fixtures/generated-do-not-edit`, and reusable TAR archives under
`tests/fixtures/generated-archives`. Names passed to `rust_fixture_*` must
be a single path component, unique within that project. The Rust function
names are retained even when Python supplies the producer:

```python
import gix
from gix import testtools

def make_fixture(state):
    path = state.path()
    if state.is_uninitialized():
        repo = gix.init(path / "repo", options=gix.OpenOptions.isolated())
        blob_id = repo.write_blob(b"fixture content")
        (path / "blob-id").write_text(str(blob_id))
    return (path / "blob-id").read_text()

fixture, blob_id = testtools.rust_fixture_read_only("objects", 1, make_fixture)
repo = gix.open_opts(fixture / "repo", gix.OpenOptions.isolated())
assert repo.find_blob(blob_id).data == b"fixture content"
```

The callback receives a `FixtureState`. `is_uninitialized()` is true when
it must create or post-process contents; it is false when the fixture comes
from the cache or an archive. The callback runs in either case to compute
its return value. Increment `version` whenever the callback's generation
behavior changes. The native lock coordinates concurrent creators, and a
failed producer leaves a marker so the next call recreates the fixture.
Python exceptions retain their original type and instance.

Read-only fixtures are a usage contract, not filesystem write protection.
Never mutate the returned directory or run Git operations that could change
its index, references, or configuration. Use isolated native open options
for reading. Post-process only while `is_uninitialized()` is true.

For a script, use `scripted_fixture_read_only("make.sh")`, or provide a
Python closure to `scripted_fixture_read_only_with_post("make.sh", 1, post)`.
The corresponding `_with_args` functions forward a list of arguments;
`scripted_fixture_read_only_with_args_with_post` accepts both. Scripts run
with native Git environment isolation. Bash fixtures can use
`gix_testtools_require_symlinks` and `fixture_has_symlinks()` for preflight.
Scripts need Git and their own interpreter installed; Python-only producers
can use native `gix` operations. Creating native archives still requires Git
to record its version in the archive metadata; ignoring archive creation
avoids that requirement.

Tests that mutate a fixture use a writable temporary directory:

```python
temporary, blob_id = testtools.rust_fixture_writable(
    "objects", 1, testtools.Creation.CopyFromReadOnly, make_fixture
)
with temporary:
    work = temporary.path()
    # Mutate only this disposable copy.
```

`TempDir.close()`, leaving its context, or dropping it removes the directory.
The cached source remains available. `Creation.CopyFromReadOnly` may call
the callback twice: once for the cache, then with a fresh state at the copy
to compute a value for its final location. This native copy mode does not
preserve symlinks. Use `Creation.Execute` for symlinks or absolute paths:
it generates directly in the final temporary directory. The scripted
writable helpers provide the same modes and post-processing overloads.

Archive generation and extraction retain native behavior. Ignore generated
directories with `tests/fixtures/generated-do-not-edit/` and, when archives
are unwanted, ignore `tests/fixtures/generated-archives/*.tar`. Set
`GIX_TEST_IGNORE_ARCHIVES=1` to regenerate locally, or use
`scripted_fixture_read_only_needs_archive()` for native archive preference.
It prefers an existing archive despite that variable and falls back to the
script if no matching archive exists. This build uses TAR, without XZ.
`GIX_TEST_FIXTURE_HASH` selects an enabled fixture hash kind.

`fixture_path()` resolves a fixture input, `fixture_bytes()` reads its bytes,
and `git(directory, arguments)` runs the native isolated Git helper for a
reference result. The argument string follows its shell-like quote parsing.
Git here is a test oracle, not a repository-operation fallback.

Native work runs detached from Python. The producer callback attaches only
while it runs; no repository state mutex is held by the binding. Callbacks
must not recursively request the same fixture, whose native file lock is
held until generation finishes. Avoid changing the process working
directory or environment while fixtures run concurrently. Python retains
ownership of signal handlers; importing the module and generating fixtures
does not install native termination handlers.

The binding covers the fixture APIs above, not every `gix-testtools` utility.
Single-archive argument variants, Git-version predicates, process-environment
guards, repository snapshots, Git daemons, and Cargo example builders are
not currently exposed.
