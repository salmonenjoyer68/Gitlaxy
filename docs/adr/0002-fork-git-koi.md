# Fork git-koi and replace the renderer

Gitlaxy reuses git-koi's MIT-licensed collection, snapshot schema, CLI shape,
GitHub Action, and GIF/PNG encoding, then replaces the koi-specific growth,
coat, markings, motion, behavior, and rendering modules with galaxy equivalents.
A clean-room rewrite would re-solve collection, error handling, deterministic
encoding, and tests without changing the product; a non-Rust stack would also
discard those tested boundaries.

Consequences: the original LICENSE and copyright stay, the snapshot format
remains compatible, and the crate and binary are renamed `gitlaxy` while the
public verbs stay `collect` and `render`.
