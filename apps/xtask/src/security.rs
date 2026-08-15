use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const REQUIRED_ATTACKS: &[&str] = &[
    "archive_bomb",
    "attachment_handling",
    "authentication",
    "awareness_instruction",
    "browser_storage",
    "command_injection",
    "credential_exfiltration",
    "cross_channel",
    "cross_profile",
    "cross_session",
    "csrf",
    "deletion_isolation",
    "delivery_isolation",
    "destructive_action",
    "device_path",
    "dns_change",
    "duplicate_event",
    "environment_injection",
    "export_disclosure",
    "forged_route",
    "kernel_isolation",
    "log_disclosure",
    "malicious_markdown",
    "malicious_media",
    "malicious_repository_content",
    "malicious_web_content",
    "mcp_isolation",
    "origin",
    "output_flood",
    "packaged_daemon",
    "packaged_desktop",
    "path_traversal",
    "payload_bound",
    "plugin_isolation",
    "protected_path",
    "rate_limit",
    "real_process_boundary",
    "redirect",
    "refinement_instruction",
    "schedule_isolation",
    "ssrf",
    "stale_lease",
    "symlink_race",
    "terminal_escape",
    "unauthenticated_access",
];

const PACKAGED_BINARIES: &[&str] = &[
    "agent-cli",
    "agent-desktop",
    "agent-tui",
    "agent-web",
    "agent-worker",
    "agentd",
    "browser-runner",
    "channel-gateway",
    "kernel-runner",
    "tool-runner",
];

struct Probe {
    package: &'static str,
    test: &'static str,
    attacks: &'static [&'static str],
}

const PROBES: &[Probe] = &[
    Probe {
        package: "keith-tool-runner-core",
        test: "tests::traversal_device_paths_size_and_cancellation_are_rejected",
        attacks: &["path_traversal", "device_path", "payload_bound"],
    },
    Probe {
        package: "keith-tool-runner-core",
        test: "tests::symlinks_and_symlink_swap_races_cannot_escape_the_capability_root",
        attacks: &["symlink_race"],
    },
    Probe {
        package: "keith-tool-runner-core",
        test: "tests::argv_is_not_reparsed_and_environment_is_minimal",
        attacks: &["command_injection", "environment_injection"],
    },
    Probe {
        package: "keith-tool-runner-core",
        test: "tests::output_flood_and_timeout_kill_the_process_tree",
        attacks: &["output_flood", "real_process_boundary"],
    },
    Probe {
        package: "keith-web",
        test: "fetch::tests::rejects_ssrf_destinations_and_non_http_schemes",
        attacks: &["ssrf"],
    },
    Probe {
        package: "keith-web",
        test: "fetch::tests::repeated_validation_catches_dns_rebinding",
        attacks: &["dns_change"],
    },
    Probe {
        package: "keith-web",
        test: "fetch::tests::redirect_targets_are_revalidated_before_connection",
        attacks: &["redirect"],
    },
    Probe {
        package: "keith-web",
        test: "browser::tests::hostile_markup_instructions_and_popups_are_neutralized",
        attacks: &["malicious_web_content", "malicious_markdown"],
    },
    Probe {
        package: "keith-web",
        test: "browser::tests::profiles_cannot_read_or_mutate_each_others_private_state",
        attacks: &["browser_storage"],
    },
    Probe {
        package: "keith-data-control",
        test: "tests::compressed_archive_bombs_are_rejected_without_expansion",
        attacks: &["archive_bomb"],
    },
    Probe {
        package: "keith-data-control",
        test: "tests::complete_lifecycle_exports_restores_deletes_rebuilds_and_isolates",
        attacks: &["deletion_isolation"],
    },
    Probe {
        package: "keith-credentials",
        test: "tests::seeded_leak_suite_scans_persistence_browser_process_export_event_log_and_diagnostics",
        attacks: &[
            "credential_exfiltration",
            "export_disclosure",
            "log_disclosure",
        ],
    },
    Probe {
        package: "keith-plugin-host",
        test: "tests::malicious_import_timeout_memory_and_crash_are_isolated_and_quarantined",
        attacks: &["plugin_isolation"],
    },
    Probe {
        package: "keith-mcp",
        test: "tests::timeout_and_malicious_output_are_bounded_and_processes_are_reaped",
        attacks: &["mcp_isolation"],
    },
    Probe {
        package: "keith-kernel-broker",
        test: "tests::isolation_resource_profiles_and_idle_reclamation_fail_closed",
        attacks: &["kernel_isolation"],
    },
    Probe {
        package: "keith-artifacts",
        test: "tests::cross_tree_and_cross_profile_access_are_denied_before_content_lookup",
        attacks: &["cross_profile"],
    },
    Probe {
        package: "keith-artifacts",
        test: "tests::spill_has_bounded_preview_media_detection_and_oversize_rejection",
        attacks: &["malicious_media"],
    },
    Probe {
        package: "keith-routing",
        test: "tests::channel_session_policies_and_unavailable_profiles_are_isolated",
        attacks: &["cross_channel", "cross_session"],
    },
    Probe {
        package: "keith-routing",
        test: "tests::channel_routes_are_durable_deterministic_and_fail_closed",
        attacks: &["forged_route"],
    },
    Probe {
        package: "keith-worker-runtime",
        test: "tests::simultaneous_claims_have_one_winner_and_expiry_advances_generation",
        attacks: &["stale_lease"],
    },
    Probe {
        package: "keith-supervisor",
        test: "renewal_loss_stops_stale_worker_and_forced_replacement_advances_generation",
        attacks: &["stale_lease", "real_process_boundary"],
    },
    Probe {
        package: "keith-daemon-core",
        test: "events::tests::acknowledgements_detach_and_command_deduplication_are_explicit",
        attacks: &["duplicate_event"],
    },
    Probe {
        package: "keith-scheduler",
        test: "tests::duplicate_claim_race_enqueues_one_action",
        attacks: &["schedule_isolation"],
    },
    Probe {
        package: "keith-delivery",
        test: "tests::acknowledgement_crash_recovers_with_honest_duplicate_state_and_receipt",
        attacks: &["delivery_isolation"],
    },
    Probe {
        package: "keith-channel-adapters",
        test: "tests::discord_gateway_inbound_dedup_attachment_isolation_and_resume_are_real",
        attacks: &["attachment_handling"],
    },
    Probe {
        package: "keith-agent-web",
        test: "security::tests::authentication_origin_csrf_and_rate_are_independent_gates",
        attacks: &[
            "authentication",
            "csrf",
            "origin",
            "rate_limit",
            "unauthenticated_access",
        ],
    },
    Probe {
        package: "keith-agent-tui",
        test: "tests::terminal_control_sequences_are_neutralized_before_rendering",
        attacks: &["terminal_escape"],
    },
    Probe {
        package: "keith-awareness",
        test: "tests::hostile_repository_instructions_remain_bounded_observed_data",
        attacks: &["awareness_instruction", "malicious_repository_content"],
    },
    Probe {
        package: "keith-evolution",
        test: "refinement::tests::reviewer_is_read_only_and_confirmed_diff_is_durable_and_undoable",
        attacks: &["refinement_instruction"],
    },
    Probe {
        package: "keith-evolution",
        test: "refinement::tests::malformed_protected_validation_no_change_and_concurrent_edits_fail_closed",
        attacks: &["protected_path"],
    },
    Probe {
        package: "keith-web",
        test: "browser::tests::every_consequential_action_requires_confirmation",
        attacks: &["destructive_action"],
    },
    Probe {
        package: "keith-agentd",
        test: "daemon_process_is_lazy_contains_crashes_and_adopts_after_restart",
        attacks: &["packaged_daemon"],
    },
    Probe {
        package: "keith-agent-desktop",
        test: "startup_existing_daemon_crash_report_restart_and_graceful_stop_use_real_processes",
        attacks: &["packaged_desktop"],
    },
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FindingLedger {
    schema_version: u16,
    findings: Vec<Finding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Finding {
    id: String,
    severity: Severity,
    status: FindingStatus,
    class: String,
    summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum FindingStatus {
    Open,
    Resolved,
}

pub fn run(root: &Path) -> Result<(), String> {
    validate_corpus()?;
    validate_findings(
        &fs::read(root.join("security/findings.json"))
            .map_err(|error| format!("security finding ledger is unavailable: {error}"))?,
    )?;
    run_command(
        root,
        "cargo",
        &["build", "--workspace", "--bins", "--release", "--locked"],
    )?;
    verify_packaged_binaries(root)?;

    let mut packages = BTreeMap::<&str, Vec<&str>>::new();
    for probe in PROBES {
        packages.entry(probe.package).or_default().push(probe.test);
    }
    for (package, expected_tests) in packages {
        let listed = listed_tests(root, package)?;
        for expected in expected_tests {
            if !listed.contains(expected) {
                return Err(format!(
                    "security probe {package}::{expected} is missing from the release test binary"
                ));
            }
        }
        run_command(
            root,
            "cargo",
            &["test", "-p", package, "--release", "--locked"],
        )?;
    }
    println!(
        "security gate passed: {} attacks, {} packaged binaries, {} real test probes",
        REQUIRED_ATTACKS.len(),
        PACKAGED_BINARIES.len(),
        PROBES.len()
    );
    Ok(())
}

fn validate_corpus() -> Result<(), String> {
    let required = REQUIRED_ATTACKS.iter().copied().collect::<BTreeSet<_>>();
    let covered = PROBES
        .iter()
        .flat_map(|probe| probe.attacks.iter().copied())
        .collect::<BTreeSet<_>>();
    if required != covered {
        let missing = required.difference(&covered).copied().collect::<Vec<_>>();
        let unexpected = covered.difference(&required).copied().collect::<Vec<_>>();
        return Err(format!(
            "security corpus mismatch; missing {missing:?}, unexpected {unexpected:?}"
        ));
    }
    let unique = PROBES
        .iter()
        .map(|probe| (probe.package, probe.test))
        .collect::<BTreeSet<_>>();
    if unique.len() != PROBES.len() {
        return Err("security corpus contains a duplicate test probe".into());
    }
    Ok(())
}

fn validate_findings(bytes: &[u8]) -> Result<(), String> {
    let ledger: FindingLedger = serde_json::from_slice(bytes)
        .map_err(|error| format!("security finding ledger is invalid: {error}"))?;
    if ledger.schema_version != 1 {
        return Err(format!(
            "security finding ledger schema {} is unsupported",
            ledger.schema_version
        ));
    }
    let mut identifiers = BTreeSet::new();
    for finding in &ledger.findings {
        if finding.id.trim().is_empty()
            || finding.class.trim().is_empty()
            || finding.summary.trim().is_empty()
            || !identifiers.insert(finding.id.as_str())
        {
            return Err("security finding ledger contains an invalid finding".into());
        }
        if finding.status == FindingStatus::Open
            && matches!(finding.severity, Severity::High | Severity::Critical)
        {
            return Err(format!(
                "release blocked by open {:?} security finding {} ({})",
                finding.severity, finding.id, finding.class
            ));
        }
    }
    Ok(())
}

fn listed_tests(root: &Path, package: &str) -> Result<BTreeSet<String>, String> {
    let output = Command::new("cargo")
        .args([
            "test",
            "-q",
            "-p",
            package,
            "--release",
            "--locked",
            "--",
            "--list",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to enumerate {package} security probes: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "failed to enumerate {package} security probes: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(str::to_owned)
        .collect())
}

fn verify_packaged_binaries(root: &Path) -> Result<(), String> {
    let release = target_directory(root).join("release");
    for binary in PACKAGED_BINARIES {
        let path = release.join(format!("{binary}{}", env::consts::EXE_SUFFIX));
        if !path.is_file() {
            return Err(format!(
                "packaged release binary is missing: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn target_directory(root: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR").map_or_else(
        || root.join("target"),
        |configured| {
            let configured = PathBuf::from(configured);
            if configured.is_absolute() {
                configured
            } else {
                root.join(configured)
            }
        },
    )
}

fn run_command(root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_complete_and_has_stable_unique_probes() {
        validate_corpus().unwrap();
    }

    #[test]
    fn serious_open_findings_block_but_resolved_findings_do_not() {
        let open = br#"{
            "schema_version": 1,
            "findings": [{
                "id": "SEC-1",
                "severity": "high",
                "status": "open",
                "class": "cross_scope",
                "summary": "cross-profile read"
            }]
        }"#;
        assert!(validate_findings(open).is_err());

        let resolved = open
            .windows(b"\"open\"".len())
            .position(|window| window == b"\"open\"")
            .map(|offset| {
                let mut bytes = open.to_vec();
                bytes.splice(
                    offset..offset + b"\"open\"".len(),
                    b"\"resolved\"".iter().copied(),
                );
                bytes
            })
            .unwrap();
        validate_findings(&resolved).unwrap();
    }
}
