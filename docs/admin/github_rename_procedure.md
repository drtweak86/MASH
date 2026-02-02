# GitHub Rename Procedure: "Team MASH"

This document captures the safe steps for renaming the GitHub account (user or organization) that currently hosts `drtweak86/MASH` to `Team MASH`. Renaming is irreversible for practical purposes (GitHub releases the old name shortly after the change), so we treat the process as a carefully choreographed migration rather than an impulsive edit.

## 1. Pre-flight checklist
1. **Audit dependent automation**: enumerate CI workflows, GitHub Apps, webhooks, GitHub Pages, release badges, and third-party services that consume `drtweak86/*`. Add a `docs/admin/github_rename_dependencies.md` entry if needed to track each URL or token that must be updated after the rename.
2. **Communicate internally**: notify the Dojo team, maintainers, and downstream consumers about the planned rename window and the expected downtime for repository URLs.
3. **Update config files before rename**: prepare PRs that update documentation, badges, or scripts that mention `drtweak86`. Keep them ready so they can be merged immediately after the rename (while the new name is still fresh).
4. **Backup GitHub settings**: export the list of collaborators, team permissions, secrets (via GitHub UI/CLI), and any critical branch protection rules.
5. **Verify the new name**: confirm `Team MASH` is available as a GitHub user or organization name ahead of the change. If the existing account is already `Team MASH`, this doc becomes a record of the process for future renames.

## 2. Executing the rename
1. **User rename**: go to Settings → Account settings → Change username (`https://github.com/settings/profile`). GitHub verifies the availability and displays impacted repositories. Accept the warning about broken URLs and proceed. Follow the official guidance: https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-github-user-account/changing-your-github-username.
2. **Organization rename**: go to Organization settings → Profile and rename (via `https://github.com/organizations/<current>/settings/profile`). GitHub warns about package visibility, automation, and GitHub Pages. Follow the instructions at https://docs.github.com/en/organizations/managing-organization-settings/managing-an-organizations-settings/renaming-an-organization.
3. **Verify ownership**: GitHub may require re-authentication or 2FA. Confirm the rename through email or in-browser prompts.
4. **Monitor GitHub notifications**: GitHub automatically creates redirects for the old repository URLs and network references, but we still treat the rename as a breakage until all references are updated.

## 3. Post-rename steps
1. **Update git remotes**: locally, run `git remote set-url origin git@github.com:Team-MASH/MASH.git` (or use the new path) for every clone, CI runner configuration, and automation that interacts with git.
2. **Refresh documentation/badges**: merge any prepared PRs that replace `drtweak86` references and publish updated badges that point to `Team-MASH/MASH` (see README badge updates).
3. **Reconfigure Webhooks and Integrations**: services like Netlify, Drone, or Zapier often store the old repo slug; update them to point to the renamed repo and verify their tokens still function.
4. **Update GitHub Pages**: rename may affect `https://drtweak86.github.io` pages; set up redirects or republish under the new organization (Team-MASH has its own `github.io` host).
5. **Notify collaborators**: send a final message reminding everyone to update their local remotes and CLI scripts (see the example commands in section 5).

## 4. Side effects & risks
| Area | Impact | Mitigation |
| --- | --- | --- |
| Repository URLs | Old URLs redirect but may break badges or automation that caches the slug. | Update all references, refresh badges, rerun documentation builds to capture new paths. |
| Git remotes | `git push`/`pull` commands referencing `drtweak86/MASH.git` will fail once the redirect expires. | Script `git remote set-url origin ...` for each clone and CI job immediately after rename. |
| GitHub Pages | Custom domain or `username.github.io` slug changes. | Update Pages settings to use the new slug and ensure DNS records still valid. |
| API tokens & Apps | OAuth apps keyed to the old organization may not recognize `Team MASH`. | Re-authorize apps or regenerate tokens if they fail; update environment variables storing repo slugs. |
| Releases & artifacts | Release download URLs change (e.g., `https://github.com/drtweak86/MASH/releases`). | Update scripts/popups to use the new `Team-MASH` path and verify release assets remain accessible via the redirect. |
| Webhooks & GitHub Actions | Some self-hosted runners may store repository paths in environment variables. | Expose the new slug via `GITHUB_REPOSITORY` automatically, but double-check any hard-coded strings inside workflows. |
| Third-party mirrors | Mirror services referencing the old slug may need updates. | Reach out to maintainers or update mirror configs. |

## 5. Rollback plan
1. **Immediate revert**: GitHub holds the old username for a short time unless someone else claims it. To roll back, rename `Team MASH` back to `drtweak86` via the same settings page. This restores the previous slug and reestablishes the original URLs.
2. **Reapply updates**: reverse any merged PRs that relied on the new `Team-MASH` slug (or keep them and adjust to match the rollback). Regenerate badges and documentation referencing `drtweak86`.
3. **Notify stakeholders**: alert the team that the rollback occurred and that they must update their remotes again.
4. **Ping GitHub support**: if the rollback is blocked because the previous name is claimed by someone else, contact GitHub Support to request reclaiming the slug (only possible for a short window). Document the support ticket number inside this repo for traceability.

## 6. Reference commands
```bash
# After rename, update remotes across clones
git remote set-url origin git@github.com:Team-MASH/MASH.git
# Update CI runners that clone the repo
rebuild-your-runner --repo=Team-MASH/MASH
# Search for the old slug inside the repo and docs
rg "drtweak86/MASH" -n
```

## 7. Key documentation links
- Changing a GitHub username: https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-github-user-account/changing-your-github-username
- Renaming an organization: https://docs.github.com/en/organizations/managing-organization-settings/managing-an-organizations-settings/renaming-an-organization
