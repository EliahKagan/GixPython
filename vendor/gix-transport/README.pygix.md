# Temporary upstream prerequisite

Source matches `gix-transport/src` at Gitoxide `pygix` HEAD
`e4d2b313b840ea0e00453791882dbc6eb7c636f6`. Its prerequisite commit is
`ab1cb834df8f48e907f81e4b7471b06b4c9f1aa0`, based on public revision
`1d7bac742f70b72ddda0c7294f4a97566b8db596`.

The patch provides per-client reqwest configuration for the pure Rust TLS
provider, with lazy initialization and redirect policy.
The prerequisite remains on the prepared local branch for Sebastian Thiel's
review. This reintegration did not modify or push that branch.

The source includes the updated base's security-audit fixes. Cargo sibling
dependencies use the pinned public Git source; workspace lints and test-only
dependencies are omitted. This source copy makes packaged builds independent
of the local checkout and unpublished revisions. Remove it after the fix lands
upstream and update the public pin. Original source licenses apply.
