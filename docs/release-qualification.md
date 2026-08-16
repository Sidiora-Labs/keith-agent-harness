# Release qualification

This is the operator-run acceptance matrix for task 13.3. Run it only against a newly assembled signed release directory, not loose Cargo binaries. Record the release manifest digest, build ID, target, operating system, provider, model, workspace path, and every command exit status.

## 1. Trust and contents

1. Obtain the expected Ed25519 public key through an independent authenticated channel.
2. Run `cargo xtask verify-release RELEASE EXPECTED_PUBLIC_KEY_HEX` from the audited source tree.
3. Confirm the verifier reports the expected version and build ID.
4. Add an unlisted file to a disposable copy and confirm verification fails.
5. Alter one listed byte in a second disposable copy and confirm verification fails.
6. Run `RELEASE/bin/agentd --build-info` and `RELEASE/bin/agent-worker --build-info`; compare both complete JSON reports with `release-manifest.json`.

## 2. Clean first installation

1. Create new absolute `STATE_ROOT`, `DATA_ROOT`, and `WORKSPACE_ROOT` paths on a clean user account or clean machine.
2. Run `RELEASE/bin/agent-desktop setup STATE_ROOT DATA_ROOT http://127.0.0.1:7341`.
3. Inspect `RELEASE/bin/agent-desktop settings STATE_ROOT` and confirm every path is inside the selected roots.
4. Configure one real provider through `agent-cli provider set --secret-env`; never pass the credential value as an argument.
5. Start packaged `agentd` with the packaged worker, then attach the packaged TUI or web application.
6. Complete a real model turn that calls a workspace write tool. Confirm the resulting file and committed assistant/tool history.

## 3. Restart and restore

1. Stop clients, terminate the daemon normally, and confirm its worker drains.
2. Restart the same packaged daemon and client. Confirm the exact selected session, assistant response, tool result, and workspace file remain available.
3. Stop the service and run `agent-desktop backup STATE_ROOT`.
4. Restore that backup into a new empty data root and confirm a modified backup or symlinked backup is rejected.
5. Point a fresh desktop state root at the restored data, restart the packaged daemon, and confirm the session resumes.

## 4. Update and rollback

1. Verify a second signed release from the same publisher key.
2. Stop the service and run `agent-desktop update STATE_ROOT SECOND_RELEASE EXPECTED_PUBLIC_KEY_HEX`.
3. Confirm a release signed by a different key is rejected.
4. Start the second release and confirm build information and session compatibility.
5. Stop it, run `agent-desktop rollback STATE_ROOT`, and confirm the earlier signed version becomes active and remains usable.

## 5. Uninstall and data choices

1. Run `agent-desktop uninstall-plan STATE_ROOT keep-user-data` and inspect every exact path before supplying its installation-specific confirmation.
2. Execute `keep-user-data`; confirm installed versions are removed while durable data remains.
3. Repeat from a fresh installation with `remove-runtime`; confirm transient runtime state is removed and documented durable data remains.
4. Move any backup that must survive outside `STATE_ROOT`, then repeat with `remove-everything`; confirm the selected state and data roots are absent.
5. Inspect the native credential store separately and remove its Keith entries through the authenticated provider settings flow or operating-system credential manager.
6. Scan the selected parent directories and confirm no undocumented Keith data remains.

Task 13.3 remains incomplete until every applicable row passes on the declared release platforms and the evidence identifies any platform-specific exclusions.
