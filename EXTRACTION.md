# Extraction and release provenance

Status Machina is published as `status-machina` 0.1.0 under MIT OR Apache-2.0.
The [initial release](https://github.com/legra-ai/status-machina/releases/tag/v0.1.0)
and [crates.io archive](https://crates.io/crates/status-machina/0.1.0) identify
source commit `5d96cd8950f6073fde67605cf485581d8c5c8114`.

Release run `42418090373` passed the shared Linux, macOS and Windows checkout and
archive checks before publication. The downloaded registry archive's
`.cargo_vcs_info.json` matches that source; its SHA-256 is
`bbb8f995b688f4219372c4f7d8740643014e4d13b821d77a2f39aa21205a46ad`.

The owner explicitly authorized replacing the unpublished initial draft/tag
before that release. Subsequent published tags and archives remain immutable.

Only generic kernel code, tests, neutral execution/retry examples and public
documentation ship. Private product topology, private source links and private
Git history are excluded. Copyright ownership is retained in the license files.
The kernel's runtime dependencies are thiserror and uuid; rhusky installs hooks
only for this repository's build context.

The repository uses Tasktree's pinned shared maintenance/release pipeline,
App identities, required checks/review, signed commits and daily updates.
Every release tests and publishes the exact revision returned by release
preparation, including recovery of an interrupted release at its existing tag.
Temporary bootstrap exceptions are closed.

Consumers adopt the versioned registry package without a local implementation
or path alias. Native Python/Node bindings are a separate deliverable described
in [the binding design](docs/bindings-design.md); neither binding is implemented
or published by this Rust release.
