# BashMenu-RS Git, PR & CI Workflow

## 1. PR-Only `main` Branch (`GH013`)
- **No Direct Pushes to `main`:** GitHub repository rules block direct `git push origin main`. Every completed task, bug fix, feature, docs update, or refactor must be pushed on a dedicated branch and merged via pull request.
- **Standard PR Lifecycle:**
  1. Create a descriptive branch (e.g., `feat/...`, `fix/...`, `docs/...`) from up-to-date `origin/main`.
  2. Commit changes (`git commit -m "..."`) and push (`git push -u origin <branch>`).
  3. Open a PR targeting `main` (`gh pr create --base main --head <branch> ...`).
  4. Watch CI checks (`gh pr checks <PR> --watch --interval 15`) until both `Build, Test & Lint` (`rust-app.yml`) and `ShellCheck Lint` (`shellcheck-app.yml`) pass.
  5. Squash-merge (`gh pr merge <PR> --squash`), delete the remote/local feature branch, and fast-forward local `main` (`git checkout main && git pull --ff-only origin main`).
- **Base Branch Drift:** If `origin/main` advances while a PR is open, rebase the PR branch onto `origin/main` (preserving any unrelated unstaged local edits), force-push with `--force-with-lease`, and re-verify checks before merging.

## 2. Large Binary Asset Push Hygiene
- **Pre-Push Blob Check:** When modifying binary assets (GIFs, images, videos in `assets/`), inspect unpushed blobs before pushing (`git rev-list --objects origin/main..HEAD`).
- **Squash Oversized Intermediates:** Never push unpushed commit history containing superseded large binaries or blobs >100 MB (which trigger HTTP 408 upload timeouts or GitHub size rejections). Squash unpushed asset commits so only the final optimized file is uploaded.
