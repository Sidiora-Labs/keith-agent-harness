## What problem does this solve?

Describe the user or operational problem. Link the Issue or Discussion when one
exists.

## What changed?

Explain the approach, the user-visible outcome, and why this scope is appropriate.

## Proof

List the exact commands and real user paths you exercised, with their results.
For Web or TUI changes, include before and after screenshots or a recording.

- [ ] Focused real tests pass
- [ ] `./keith check` or the relevant stricter gate passes
- [ ] UI, process, API, provider, browser, or container behavior was exercised when applicable
- [ ] Blocked or untested paths are identified explicitly

## Security and compatibility

Describe authority, approval, credential, profile, storage, API, protocol,
migration, deployment, and rollback implications. Write `none` only after
checking each relevant boundary.

## Final checklist

- [ ] The change is focused and contains no unrelated refactors
- [ ] Tests and documentation cover changed behavior
- [ ] No secrets, personal data, repository-local targets, or generated junk are included
- [ ] I reviewed and understand any AI-assisted changes
- [ ] The pull-request description reflects the final implementation and evidence
