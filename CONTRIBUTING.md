# Contributing

Create a branch for each change and open a pull request into `main`. Do not push changes directly to `main`. Individual commit messages are unrestricted.

Name branches `<type>/<issue-number>-<short-description>`, using lowercase words
separated by hyphens. For example, `fix/3-profile-trainer-ui` or
`feat/12-session-export`. Use the relevant GitHub issue number and link the PR
to that issue with `Closes #<number>` when merging should complete the issue.
This naming convention is documented guidance; the automated check enforces
PR titles, not branch names.

Pull request titles must use `<type>[optional scope][!]: <description>`.
Examples:

- `feat: add session export`
- `fix(ui): show trainer connection progress`
- `refactor(api)!: change session response`

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
Use a colon followed by a space and a nonempty description. The PR title workflow validates titles when pull requests are opened, edited, or updated.

Repository administrators should protect `main` by requiring pull requests and the `PR title` status check, with no bypasses. Required approvals can remain disabled for solo development. Force pushes and deletions should remain disabled on `main`. These settings are configured on GitHub, separately from the workflow file.

Rebasing a feature branch onto `main` does not require allowing force pushes to `main`.
