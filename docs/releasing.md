# Releasing batuta-cli

Releases are deliberately gated by a human-reviewed pull request. A push to
`main` opens or updates the single `release-plan` branch and its
`release: vX.Y.Z` pull request. Only merging that pull request creates the
version tag; the tag then starts the generated cargo-dist workflow.

## Required repository setup

Create a repository secret named **`RELEASE_TOKEN`** before merging the first
release PR. The secret must contain a repo-scoped personal access token owned
by a maintainer:

- Fine-grained PAT: select only this repository and grant repository
  permissions **Contents: Read and write** and **Pull requests: Read and
  write**.
- Classic PAT fallback: grant the **`repo`** scope.

The workflow uses this PAT both to maintain the release branch/PR and to push
the version tag. Do not replace it with `GITHUB_TOKEN`: GitHub suppresses
downstream workflow events for tags pushed with the default token, so
`release.yml` would not run.

In **Settings → Actions → General → Workflow permissions**, allow GitHub
Actions to create pull requests. Protect `main` so the `release-plan` PR
requires a human review and merge; do not enable automatic merging for it.

## Release procedure

1. Merge normal changes to `main`. The Release plan workflow runs
   `git-cliff --bump`, updates the workspace version and `CHANGELOG.md`, and
   opens or refreshes the standing release PR.
2. Review the version, generated changelog section, and CI results. Do not
   create a tag manually.
3. Merge the PR. The merge-only job validates its title and changelog, then
   pushes `vX.Y.Z` using `RELEASE_TOKEN`.
4. The generated `release.yml` verifies that the tag commit belongs to that
   merged release PR, builds `batuta` for Linux and Intel/Apple Silicon macOS,
   and publishes the Release only after every platform succeeds.
5. Download an archive and its `.sha256` file from the Release and verify it:

   ```console
   sha256sum -c batuta-<platform>.tar.xz.sha256
   ```

The `v0.1.0-beta.1` tag annotation is retained as historical record. Its
message mentions delivery 1 even though the tagged tree also contains
delivery 2; release automation does not rewrite existing tags.
