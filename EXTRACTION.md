# Extraction candidate

This is a standalone candidate for `legra-ai/status-machina`, version `0.1.0`, under MIT OR Apache-2.0, following the Tasktree maintenance and release template. Version 0.1.0 is a proposed initial package version, not a published release.

Only generic kernel code, generic integration tests, neutral execution/retry examples and public documentation are included. No private repository history is copied. Product-derived startup graph snapshots and private source links are excluded. Existing copyright ownership is retained in the license files.

The Rust kernel depends only on thiserror and uuid. The repository-only hook installer follows Tasktree. Python and Node bindings are assessed in docs/bindings-design.md and require a separate API and package verification slice.

Repository configuration target: squash-only merges; delete merged branches; issues enabled; wiki/projects disabled; inherited signed commits; required prepare/test/Conventional Commit checks; owner review; daily dependency/security updates. The workflow template uses the shared release actions and tests the packaged archive. No credentials, repository visibility changes or publication are implied by this local configuration.

Before public publication, verify repository settings and release writer identity, review the complete public file tree, confirm package availability, obtain owner approval for publication and verify the exact published archive. Legra adoption and deletion of its in-tree implementation follow verification of the external package.

Private extraction CI uses read-only release planning and validates the triggering source revision. Release activation must install the authorized release writer and switch validation/publication to its returned exact revision; existing draft tags must remain immutable.

Release activation uses the pinned shared cargo-maintenance workflow and release-writer App, as in the current Tasktree reference. PR validation is allowed while private; main activation requires public visibility. The initial draft/tag must resume unchanged before later releases.
