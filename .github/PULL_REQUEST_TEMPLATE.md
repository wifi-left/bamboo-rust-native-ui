<!--
Thanks for the pull request. Keep it focused: one behaviour change, its tests,
and the documentation it needs. Remove this comment before submitting.
-->

## Summary

<!-- What changed, and why? Link the issue it closes. -->

Closes #

## Type

- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Translation
- [ ] Cross-platform support
- [ ] Refactor / performance
- [ ] Tooling / build

## What changed

<!-- A short walkthrough of the diff. Call out public API, XML/theme schema,
     paint-path, or platform-layer changes explicitly. -->

## Verification

Run what applies; say in the notes which checks were skipped and why.

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --all-targets`
- [ ] `cargo build` — 0 warnings
- [ ] `cargo test`
- [ ] `cargo run -p bamboo-native-ui-showcase -- --self-test`
- [ ] `cargo run -p bamboo-native-ui-showcase -- --a11y-dump`
- [ ] `zensical build -s -f mkdocs.yml`
- [ ] `zensical build -s -f mkdocs.zh.yml`
- [ ] Documentation updated in both `docs/en/` and `docs/zh/` (user-facing changes)
- [ ] Relative links checked

## Screenshots or recordings

<!-- For visual changes: before/after, light and dark, 100% and 150% DPI. -->

## Notes

<!--
- Breaking changes or migration steps.
- New dependencies and why they are needed.
- AI assistance: say so, and describe how the result was verified.
- Anything reviewers should look at first.
-->
