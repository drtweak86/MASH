# GitHub Account/Organization Rename Procedure (to "Team MASH")

This document describes a safe procedure for renaming a GitHub **user** account (e.g. `drtweak86`) or a GitHub **organization** to a new name (e.g. `Team-MASH` / `team-mash`).

Scope: documentation only. Do **not** execute the rename while following this guide.

## Key GitHub Docs (source of truth)

- Username rename procedure: https://docs.github.com/en/account-and-profile/how-tos/setting-up-and-managing-your-personal-account-on-github/managing-your-personal-account/changing-your-username
- Username rename impacts: https://docs.github.com/en/account-and-profile/concepts/username-changes
- Organization rename procedure + impacts: https://docs.github.com/en/organizations/managing-organization-settings/renaming-an-organization

## Preflight Checklist (Before Renaming)

1. Choose the target name and its exact casing:
   - GitHub names are case-insensitive for uniqueness, but casing matters in URLs and branding.
   - Confirm what the canonical namespace will be: `Team-MASH` vs `team-mash` vs `TeamMASH`.
2. Inventory dependencies that reference the current owner/name:
   - Git remotes in contributor clones (`origin` URLs).
   - CI/CD secrets (webhooks, deploy keys, GitHub Apps).
   - External integrations (Docker image pulls, package dependencies, API clients, badges).
   - Documentation links, README badges, website links.
3. Inventory GitHub-native features that are sensitive to renames:
   - GitHub Pages (CNAME/custom domain), Actions, Packages/Container registry, Marketplace listings.
   - CODEOWNERS entries and team mentions (`@org/team`).
4. Pick a quiet time window:
   - GitHub redirects can take a few minutes to propagate.
   - Plan for a short period where API calls and cached links may fail.
5. Communication plan:
   - Post a short announcement to contributors with the rename date/time and the new `git remote set-url` commands.

## What Changes (and What Does Not)

### Redirect behavior (repositories)

GitHub creates redirects from the old namespace to the new one for repository URLs, but redirects can be overridden if someone else claims the old name and creates a repo with the same name. Updating remotes is recommended. See the GitHub docs links above.

### Profile links

Old profile URLs (e.g. `https://github.com/OLDNAME`) may return 404 after the rename (GitHub explicitly calls this out for both users and orgs).

### API clients

API requests using the old org/user name can return 404 after rename; update integrations.

### Gists (user rename)

Public/secret gist URLs change and old links can return 404. Any shared links must be updated.

### CODEOWNERS and mentions

- CODEOWNERS that contain the old username must be updated.
- For org renames, team mention redirects are not automatic for `@org/team` patterns.

### Packages / container images (org rename)

GitHub may transfer packages and container images to the new namespace, but downstream consumers may break if they depend on the old namespace. GitHub may also permanently retire certain old name combinations for high-traffic Actions repos and for popular container images.

## Procedure: Renaming a User Account

Follow GitHub’s official steps and read the warnings:
1. Open GitHub Settings.
2. Go to **Account**.
3. Under **Change username**, select **Change username**.
4. Read warnings and confirm.
5. Enter the new username and confirm.

Immediately after:
1. Update documentation and badges in this repo that reference the old owner.
2. Update all contributor instructions:
   - `git remote set-url origin git@github.com:NEWNAME/MASH.git`
   - or HTTPS equivalent.
3. Update automation/integrations:
   - GitHub CLI scripts, API clients, webhooks, OAuth apps, GitHub Apps.
4. Search and update CODEOWNERS references (if applicable).
5. Verify:
   - `git clone` works using the new URL.
   - Existing clones can `git fetch` after remote update.
   - Any external links used in docs resolve correctly.

## Procedure: Renaming an Organization

Follow GitHub’s official steps and read the warnings:
1. Open **Organizations** from the profile menu.
2. Select the org, go to **Settings**.
3. In “Danger zone”, choose **Rename organization**.
4. Read warnings and confirm.
5. Enter the new name and confirm.

Immediately after:
1. Update repo remotes (same as user rename guidance).
2. Update API clients and webhooks that reference the old org name.
3. Update team mentions and CODEOWNERS files across repos.
4. Validate package and container publishing:
   - confirm publish destinations and consumer docs now reference the new namespace.
5. Validate GitHub Actions behavior, especially if any repo is listed on GitHub Marketplace.

## Post-Rename Validation Checklist

- Repository redirects:
  - Old repo URL redirects to the new repo URL.
  - Cloning new URL works over SSH and HTTPS.
- Contributor workflow:
  - Existing clones can update remotes and push.
- Integrations:
  - Any CI pipelines and webhooks that depend on the namespace still function.
  - Any package/container consumers have updated docs and continue to work.
- Documentation:
  - Search-and-replace old owner/org name in docs.
  - Verify badges and links.

## Rollback / Recovery Plan

Important: GitHub warns that after a rename, the **old name becomes available for someone else to claim**. If the old name is claimed, a rollback may be impossible or partially broken.

Rollback options (best-effort):
1. If the old name is still available and GitHub allows it, rename back to the original name using the same rename procedure.
2. If the old name was claimed by someone else:
   - You may not be able to reclaim it without GitHub Support and (potentially) trademark claims.
   - Redirects may stop working if the new owner creates conflicting repositories.

Mitigation (recommended instead of relying on rollback):
- Treat renames as effectively permanent.
- Update all remotes/integrations promptly after rename.
- Keep a checklist of all external dependencies and verify them during the maintenance window.

