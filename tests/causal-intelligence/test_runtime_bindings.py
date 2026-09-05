"""Hosted-provider binding capture, correction, and actual workspace dispatch.

The original runtime baseline supplies the real daemon, worker, authenticated
Web transport, and transparent OpenRouter proxy. Target bindings and task
dependencies must be produced by ordinary user turns and production tools.
"""

from __future__ import annotations

import hashlib
import http.cookiejar
import importlib.util
import json
import os
from pathlib import Path
import unittest
import urllib.error
import urllib.parse
import urllib.request


_spec = importlib.util.spec_from_file_location(
    "_causal_bindings_baseline", Path(__file__).with_name("test_runtime_baseline.py"))
baseline = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(baseline)
baseline.TARGET = Path("/tmp/keith-causal-bindings-build")


def canonical_digest(value):
    encoded = json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


class RuntimeBindings(baseline.RuntimeBaseline):
    replicate = "unselected"

    @classmethod
    def prepare_runtime(cls):
        # Each replicate owns a fresh profile, workspace and daemon stack. Share
        # only the compiled binaries, never bindings or conversation history.
        artifact_root = os.environ.get("KEITH_QUALIFICATION_ARTIFACT_DIR")
        cls.report_root = Path(artifact_root) if artifact_root else None
        if artifact_root:
            os.environ["KEITH_QUALIFICATION_ARTIFACT_DIR"] = str(cls.report_root / cls.replicate)
        try:
            super().prepare_runtime()
        finally:
            if artifact_root:
                os.environ["KEITH_QUALIFICATION_ARTIFACT_DIR"] = artifact_root
        cls.report["replicate"] = cls.replicate
        cls.report["qualification_scope"] = {
            "task": "2.2",
            "required_test_scope": [
                "natural user mapping captured by actual source-cited memory tool",
                "correction retains exact entity/property and original provenance",
                "fresh-session exact lookup reaches actual provider context",
                "bound read executes at the current permitted path",
                "durable binding and current dependent read survive restart",
            ],
            "not_claimed": [
                "10,000-distractor domain cases by this hosted-provider group",
                "all adversarial admission cases by this hosted-provider group",
                "semantic retrieval or later full automatic activation qualification",
                "external truth established by an inferred binding association",
                "mixed bound-input read and new-output write parity before typed operation roles",
            ],
            "current_dispatch_limit": (
                "Inspectable bound target slots are read.path and web_fetch.url. With required "
                "dependencies or unavailable alias lookup, write/list/search/review_content, "
                "bash/browser/kernel, and MCP/plugin routes are refused. This suite qualifies "
                "exact bound reads; it does not qualify "
                "ordinary read-input/write-new-output workflows under that guard."),
        }
        cls.report["binding_observations"] = []
        cls.report["restart_observations"] = []
        cls.report["write_attempts"] = []

    @classmethod
    def write_report(cls):
        try:
            super().write_report()
        finally:
            source = cls.artifacts / "baseline.json"
            if source.exists():
                source.rename(cls.artifacts / "bindings.json")
            if cls.report_root is not None:
                reports = sorted(cls.report_root.glob("replicate-*/bindings.json"))
                aggregate = {key: cls.report[key] for key in ("run_id", "case_id", "source_digest")}
                aggregate["required_replicates"] = 3
                aggregate["replicates"] = [
                    {"path": str(path.relative_to(cls.report_root)), "sha256": baseline.digest(path),
                     "report": json.loads(path.read_text())} for path in reports]
                for item in aggregate["replicates"]:
                    if any(item["report"].get(key) != aggregate[key]
                           for key in ("run_id", "case_id", "source_digest")):
                        raise AssertionError("binding replicates belong to different proof invocations")
                (cls.report_root / "bindings.json").write_text(json.dumps(aggregate, indent=2) + "\n")

    @classmethod
    def session_entries(cls, session):
        entries = super().session_entries(session)
        seen = set()
        for entry in entries:
            if entry["id"] in seen:
                raise AssertionError("canonical binding history duplicated an entry identity")
            expected = canonical_digest({key: value for key, value in entry.items() if key != "checksum"})
            if entry["checksum"] != expected:
                raise AssertionError("canonical binding history checksum did not verify")
            if entry["parent_id"] is not None and entry["parent_id"] not in seen:
                raise AssertionError("canonical binding history lost its preceding parent")
            seen.add(entry["id"])
        return entries

    @classmethod
    def ask_with_entries(cls, session, prompt):
        prior = {entry["id"] for entry in cls.session_entries(session)}
        snapshot, answer = cls.ask(session, prompt)
        entries = [entry for entry in cls.session_entries(session) if entry["id"] not in prior]
        ingresses = [entry for entry in entries
                     if entry["payload"].get("payload") == "user_message"
                     and baseline.message_text(entry["payload"]["message"]["content"]) == prompt]
        if len(ingresses) != 1:
            raise AssertionError("binding turn did not have one exact committed user source")
        cls.report.setdefault("turn_summaries", []).append({
            "phase": cls.report.get("journey_phase"), "session_id": session,
            "turn_id": snapshot["terminal"]["turn_id"],
            "source_entry_id": ingresses[0]["id"], "source_checksum": ingresses[0]["checksum"],
            "tools": [{"name": entry["payload"]["name"],
                       "argument_keys": sorted(entry["payload"]["arguments"]),
                       "call_id": entry["payload"]["call_id"]}
                      for entry in entries if entry["payload"].get("payload") == "tool_call"],
            "tool_results": [{"call_id": entry["payload"]["call_id"],
                              "is_error": entry["payload"]["is_error"],
                              "failure_code": ((entry["payload"].get("failure") or {})
                                               .get("error") or {}).get("code"),
                              "effect_state": (entry["payload"].get("failure") or {})
                                              .get("effect_state"),
                              "automatic_retry": ((entry["payload"].get("failure") or {})
                                                  .get("retry") or {}).get("automatic")}
                             for entry in entries if entry["payload"].get("payload") == "tool_result"],
        })
        return snapshot, answer, entries, ingresses[0]

    @staticmethod
    def tool_pairs(entries, name):
        pairs = []
        for index, entry in enumerate(entries):
            call = entry["payload"]
            if call.get("payload") != "tool_call" or call["name"] != name:
                continue
            results = [candidate for candidate in entries[index + 1:]
                       if candidate["payload"].get("payload") == "tool_result"
                       and candidate["payload"]["call_id"] == call["call_id"]]
            if len(results) != 1:
                raise AssertionError("actual tool call does not have one subsequent committed result")
            pairs.append((entry, results[0]))
        return pairs

    def successful_json(self, pair):
        call, result = pair
        self.assertFalse(result["payload"]["is_error"],
                         "required memory tool returned an execution error")
        self.assertIsNone(result["payload"].get("failure"))
        raw = baseline.message_text(result["payload"]["content"])
        value = json.loads(raw)
        self.assertIsInstance(value, dict)
        self.report["binding_observations"].append({
            "boundary": "actual_committed_tool_result",
            "tool_name": call["payload"]["name"],
            "call_id": call["payload"]["call_id"],
            "call_entry_id": call["id"], "call_checksum": call["checksum"],
            "result_entry_id": result["id"], "result_checksum": result["checksum"],
            "result_sha256": hashlib.sha256(raw.encode()).hexdigest(),
        })
        return value

    def assert_write_recovery(self, entries, expected_tool, ingress, *,
                              expected_owner=None, expected_binding=None):
        writes = {"memory_create", "memory_correct", "memory_forget"}
        permitted = writes | {"memory_search", "memory_get", "memory_context"}
        calls = [entry for entry in entries if entry["payload"].get("payload") == "tool_call"]
        self.assertFalse([entry["payload"]["name"] for entry in calls
                          if entry["payload"]["name"] not in permitted],
                         "source turn escaped its requested memory-only operations")
        self.assertEqual(len({entry["payload"]["call_id"] for entry in calls}), len(calls),
                         "tool intents reused a call identity")
        pairs = sorted((pair for name in writes for pair in self.tool_pairs(entries, name)),
                       key=lambda pair: entries.index(pair[0]))
        observations = []
        for call, result in pairs:
            arguments = call["payload"]["arguments"]
            failure = result["payload"].get("failure") or {}
            observations.append({
                "phase": self.report["journey_phase"],
                "attempt": len(observations) + 1,
                "tool_name": call["payload"]["name"], "call_id": call["payload"]["call_id"],
                "call_entry_id": call["id"], "call_checksum": call["checksum"],
                "result_entry_id": result["id"], "result_checksum": result["checksum"],
                "argument_keys": sorted(arguments), "has_binding": bool(arguments.get("binding")),
                "is_error": result["payload"]["is_error"],
                "failure_code": (failure.get("error") or {}).get("code"),
                "effect_state": failure.get("effect_state"),
                "automatic_retry": (failure.get("retry") or {}).get("automatic"),
                "source_entry_matches_ingress": arguments.get("source_entry_id") == ingress["id"],
                "owner_id_matches_expected": (arguments.get("evidence_id") == expected_owner
                                               if expected_owner is not None else None),
                "owner_id_is_current_source_entry": arguments.get("evidence_id") == ingress["id"],
                "owner_id_is_prior_source_evidence": (
                    arguments.get("evidence_id") == expected_binding["evidence_id"]
                    if expected_binding is not None else None),
                "expected_binding_matches": (arguments.get("expected_binding") == expected_binding
                                             if expected_binding is not None else None),
            })
        # Preserve every attempt before checking it, including malformed/unbound
        # writes and unknown effects that must keep this journey nonpassing.
        self.report["write_attempts"].extend(observations)
        successes = []
        for pair, observed in zip(pairs, observations):
            if observed["is_error"]:
                self.assertIn(observed["effect_state"], {"not_started", "not_committed"},
                              "failed memory write lacks an explicit no-commit effect state")
                self.assertIs(observed["automatic_retry"], False,
                              "failed memory write permitted automatic retry")
            else:
                self.assertIsNone(pair[1]["payload"].get("failure"))
                successes.append(pair)
        self.assertEqual(len(successes), 1,
                         "source turn must commit exactly one successful memory write")
        successful = successes[0]
        self.assertEqual(successful[0]["payload"]["name"], expected_tool)
        self.assertTrue(successful[0]["payload"]["arguments"].get("binding"),
                        "successful memory write omitted the required exact binding")
        self.report["binding_observations"].append({
            "boundary": "explicit_write_recovery", "phase": self.report["journey_phase"],
            "attempt_count": len(pairs), "failed_attempt_count": len(pairs) - 1,
            "successful_write_count": 1,
            "successful_call_id": successful[0]["payload"]["call_id"],
            "failed_attempts_declared_no_commit": True,
            "failed_attempts_disallow_automatic_retry": True,
        })
        return successful

    def assert_source_cited_mapping(self, call, ingress, value, *, alias=None):
        arguments = call["payload"]["arguments"]
        source_text = baseline.message_text(ingress["payload"]["message"]["content"])
        self.assertEqual(arguments["source_entry_id"], ingress["id"])
        quote = arguments["evidence_quote"]
        self.assertIsInstance(quote, str)
        self.assertTrue(quote)
        self.assertIn(quote, source_text)
        binding = arguments["binding"]
        self.assertEqual(binding["value_quote"], value)
        self.assertIn(value, quote)
        if alias is not None:
            self.assertEqual(binding["entity"], {"mode": "new_alias", "alias": alias})
            self.assertEqual(binding["property"], "status_path")
            self.assertEqual(binding["target_kind"], "workspace_path")
        self.report["binding_observations"].append({
            "boundary": "natural_source_mapping",
            "source_entry_id": ingress["id"], "source_checksum": ingress["checksum"],
            "source_quote_sha256": hashlib.sha256(quote.encode()).hexdigest(),
            "tool_call_id": call["payload"]["call_id"],
            "captured_value": value,
            "association_truth_claim": "attributed mapping; no external truth certification",
        })

    def assert_actual_read(self, session, snapshot, answer, entries, path, content, reference):
        binding_id = reference["binding_id"]
        reads = self.tool_pairs(entries, "read")
        self.assertEqual(len(reads), 1, "dependent task must execute one actual workspace read")
        call, result = reads[0]
        self.assertEqual(call["payload"]["arguments"]["path"], path)
        self.assertFalse(result["payload"]["is_error"])
        self.assertIsNone(result["payload"].get("failure"))
        self.assertEqual(baseline.message_text(result["payload"]["content"]), content)
        self.assertIn(content.strip(), answer["text"], "final did not contain the file's fresh value")
        permitted = {"memory_search", "memory_get", "memory_context", "read"}
        self.assertFalse([entry["id"] for entry in entries
                          if entry["payload"].get("payload") == "tool_call"
                          and entry["payload"]["name"] not in permitted],
                         "binding fixture escaped its requested memory lookup and file read")
        lookups = self.tool_pairs(entries, "memory_context")
        self.assertTrue(lookups, "natural task did not exercise exact memory-context lookup")
        read_position = entries.index(call)
        lookup_ids = {
            lookup["payload"]["call_id"] for lookup, response in lookups
            if entries.index(response) < read_position and not response["payload"]["is_error"]
        }
        self.assertTrue(lookup_ids, "binding lookup did not precede dependent dispatch")
        captures = self.turn_requests[(session, snapshot["terminal"]["turn_id"])]
        included = []
        for capture in captures:
            messages = capture["request"].get("messages", [])
            already_in_history = any(
                item.get("id") == call["payload"]["call_id"]
                for message in messages for item in message.get("tool_calls", []))
            if already_in_history:
                continue
            self.assertNotIn(content.strip(), json.dumps(capture["request"]),
                             "file-only nonce appeared in provider context before its actual read")
            for message in messages:
                text = baseline.message_text(message.get("content"))
                if (message.get("role") == "tool"
                        and message.get("tool_call_id") in lookup_ids
                        and binding_id in text and path in text):
                    included.append(capture["request_sha256"])
        self.assertTrue(included, "resolved binding did not reach provider context before actual read")
        requirements = [entry for entry in entries
                        if entry["payload"].get("payload") == "required_object_bindings"]
        admissions = [entry for entry in entries
                      if entry["payload"].get("payload") == "binding_admission"
                      and entry["payload"]["admission"]["call_id"] == call["payload"]["call_id"]]
        self.assertEqual(len(admissions), 1, "read needs one canonical frozen admission")
        admitted = admissions[0]
        admission = admitted["payload"]["admission"]
        self.assertLess(entries.index(call), entries.index(admitted))
        self.assertLess(entries.index(admitted), entries.index(result),
                        "frozen admission must precede the actual tool result")
        self.assertEqual(admission["arguments_digest"], canonical_digest(call["payload"]["arguments"]))
        self.assertEqual(admission["turn_id"], snapshot["terminal"]["turn_id"])
        self.assertEqual(admission["tool_name"], "read")
        self.assertEqual(admission["scope"]["profile_id"], self.profile["id"])
        self.assertEqual(admission["scope"]["workspace_id"], self.profile["workspace_id"])
        self.assertEqual(admission["scope"]["session_id"], session)
        obligations = [entry["payload"] for entry in entries
                       if entry["payload"].get("payload") == "turn_obligation"
                       and entry["payload"]["turn_id"] == admission["turn_id"]]
        self.assertTrue(obligations)
        self.assertTrue(all(item["action_id"] == admission["scope"]["action_id"] for item in obligations))
        target = {"kind": "workspace_path", "tool_name": "read", "argument_name": "path"}
        required = {"key": reference["key"], "target": target}
        self.assertIn(required, admission["required"])
        self.assertEqual(admission["bindings"], [{"reference": reference, "target": target}])
        self.assertTrue(any(
            entries.index(entry) < entries.index(admitted)
            and entry["payload"]["record"]["scope"] == admission["scope"]
            and required in entry["payload"]["record"]["required"]
            for entry in requirements), "required key was not durably recorded before admission")
        self.report["binding_observations"].append({
            "boundary": "actual_context_and_dependent_read",
            "session_id": session, "turn_id": snapshot["terminal"]["turn_id"],
            "binding_id": binding_id, "read_call_id": call["payload"]["call_id"],
            "read_call_entry_id": call["id"], "read_call_checksum": call["checksum"],
            "read_result_entry_id": result["id"], "read_result_checksum": result["checksum"],
            "actual_path": path,
            "actual_content_sha256": hashlib.sha256(content.encode()).hexdigest(),
            "provider_request_hashes_before_read": included,
            "final_id": snapshot["terminal"]["final_id"],
            "context_inclusion_is_dispatch_proof": False,
            "frozen_admission_entry_id": admitted["id"],
            "frozen_admission_checksum": admitted["checksum"],
            "frozen_binding_reference": reference,
            "arguments_digest": admission["arguments_digest"],
            "required_key_committed_before_dispatch_result": True,
        })
        return call

    def canonical_vault(self, *, allow_missing=False):
        path = self.workspace / ".keith/.keith/memory-vault.jsonl"
        self.assertFalse(path.is_symlink())
        self.assertTrue(path.resolve().is_relative_to(self.workspace.resolve()))
        try:
            with path.open("rb") as stream:
                raw = stream.read(16 * 1024 * 1024 + 1)
        except FileNotFoundError:
            if allow_missing:
                return [], b""
            raise
        self.assertLessEqual(len(raw), 16 * 1024 * 1024)
        self.assertTrue(not raw or raw.endswith(b"\n"),
                        "canonical vault has an incomplete mutation tail")
        events = [json.loads(line) for line in raw.splitlines() if line.strip()]
        previous = None
        for sequence, event in enumerate(events, 1):
            self.assertEqual(event["profile_id"], self.profile["id"])
            self.assertEqual(event["sequence"], sequence)
            self.assertEqual(event["previous_digest"], previous)
            self.assertEqual(event["digest"], canonical_digest(
                {key: value for key, value in event.items() if key != "digest"}))
            previous = event["digest"]
        return events, raw

    def canonical_binding(self, reference, source_session, ingress, value, prior=None, *,
                          receipts, alias):
        events, complete = self.canonical_vault()
        self.assertEqual([event["digest"] for event in events[:len(self.initial_vault_digests)]],
                         self.initial_vault_digests, "the pre-journey canonical prefix changed")
        records, bindings, commitments = {}, {}, {}
        binding_events = []
        for event in events:
            mutation = event["mutation"]
            kind = mutation["mutation"]
            after_start = event["sequence"] > len(self.initial_vault_digests)
            if kind == "observed":
                evidence = mutation["evidence"]
                self.assertNotIn(evidence["id"], records, "evidence identity was overwritten")
                if after_start:
                    self.assertNotIn(evidence["source_kind"],
                                     {"durable_memory", "daily_memory", "current_state"},
                                     "standalone memory mutation escaped the exact bound write")
                records[evidence["id"]] = dict(evidence)
            elif kind == "superseded":
                self.assertFalse(after_start, "standalone correction escaped the bound lineage")
                records[mutation["prior_id"]]["validity"] = "superseded"
                records[mutation["prior_id"]]["superseded_by"] = mutation["replacement"]["id"]
                records[mutation["replacement"]["id"]] = dict(mutation["replacement"])
            elif kind == "binding_associated":
                binding = mutation["binding"]
                self.assertNotIn(binding["reference"]["binding_id"], bindings,
                                 "binding identity was overwritten")
                self.assertEqual(binding["reference"]["revision"], event["sequence"])
                bindings[binding["reference"]["binding_id"]] = binding
                binding_events.append(event)
                owner = binding["memory"]
                self.assertNotIn(owner["id"], records, "bound owner identity was overwritten")
                records[owner["id"]] = dict(owner)
                if binding.get("prior"):
                    old_owner = bindings[binding["prior"]["binding_id"]]["owner_memory_id"]
                    self.assertEqual(owner["supersedes"], old_owner)
                    records[old_owner]["validity"] = "superseded"
                    records[old_owner]["superseded_by"] = owner["id"]
            elif kind == "source_committed":
                item = mutation["reference"]
                self.assertEqual(item["profile_id"], self.profile["id"])
                key = (item["session_id"], item["entry_id"])
                self.assertEqual(commitments.get(key, item["checksum"]), item["checksum"])
                commitments[key] = item["checksum"]
            elif kind in ("deleted", "disputed"):
                self.assertFalse(after_start, "destructive memory mutation escaped the bound write")
                records[mutation["evidence_id"]]["validity"] = kind
            elif kind == "provenance_annotated":
                record = records[mutation["evidence_id"]]
                if after_start:
                    self.assertNotIn(record["source_kind"],
                                     {"durable_memory", "daily_memory", "current_state"},
                                     "a hidden annotation changed canonical memory ownership")
                record["causal"] = mutation["metadata"]
                if mutation.get("authority") is not None:
                    self.assertEqual(mutation["authority"], "derived_inference")
                    record["authority"] = mutation["authority"]
            elif kind == "sensitivity_changed":
                self.assertFalse(after_start, "memory policy mutation escaped the bound write")
                records[mutation["evidence_id"]]["sensitivity"] = mutation["sensitivity"]
            else:
                self.fail("unsupported canonical vault mutation in binding proof")
        expected_lineage = [prior, reference] if prior else [reference]
        self.assertEqual([event["mutation"]["binding"]["reference"] for event in binding_events],
                         expected_lineage,
                         "canonical history contains an extra entity, binding or revision")
        self.assertEqual([receipt["binding"] for receipt in receipts], expected_lineage)
        for index, (event, receipt) in enumerate(zip(binding_events, receipts)):
            committed = event["mutation"]["binding"]
            self.assertEqual(committed["memory"], receipt["evidence"],
                             "canonical bound memory differs from the successful write receipt")
            self.assertEqual(committed["prior"], expected_lineage[index - 1] if index else None)
            self.assertEqual(committed["reference"]["key"], reference["key"])
            self.assertEqual(committed["owner_memory_id"], receipt["evidence"]["id"])
            self.assertEqual(committed["owner_memory_digest"], receipt["evidence"]["content_digest"])
            self.assertEqual(committed["scope"]["profile_id"], self.profile["id"])
            self.assertEqual(committed["scope"]["workspace_id"], self.profile["workspace_id"])
            self.assertEqual(committed["scope"]["session_id"], source_session)
            self.assertEqual(committed["entity"], {
                "id": reference["key"]["entity_id"], "workspace_id": self.profile["workspace_id"],
                "alias": alias,
            } if index == 0 else None, "the exact lineage introduced an alternate entity")
            source = records[committed["reference"]["evidence_id"]]
            self.assertEqual(source["content_digest"], committed["reference"]["evidence_digest"])
            self.assertEqual(source["content_digest"], hashlib.sha256(source["text"].encode()).hexdigest())
            self.assertEqual(source["authority"], "user_asserted")
            self.assertEqual(source["validity"], "active")
            expected_owner = dict(receipt["evidence"])
            if index + 1 < len(receipts):
                expected_owner["validity"] = "superseded"
                expected_owner["superseded_by"] = receipts[index + 1]["evidence"]["id"]
            self.assertEqual(records[committed["owner_memory_id"]], expected_owner,
                             "a hidden mutation changed the canonical owner beyond exact supersession")
        binding = bindings[reference["binding_id"]]
        self.assertEqual(binding["reference"], reference)
        self.assertEqual(binding["prior"], prior)
        self.assertEqual(binding["scope"]["profile_id"], self.profile["id"])
        self.assertEqual(binding["scope"]["workspace_id"], self.profile["workspace_id"])
        self.assertEqual(binding["scope"]["session_id"], source_session)
        self.assertEqual(binding["association_origin"], "inferred")
        source = records[reference["evidence_id"]]
        self.assertEqual(source["authority"], "user_asserted")
        self.assertEqual(source["content_digest"], reference["evidence_digest"])
        self.assertEqual(source["source_session"], source_session)
        self.assertIn((ingress["id"], ingress["checksum"]),
                      list(zip(source["source_entries"], source["source_digests"])))
        self.assertEqual(commitments[(source_session, ingress["id"])], ingress["checksum"])
        span = binding["source_span"]
        self.assertEqual(source["text"].encode()[span["start"]:span["end"]].decode(), value)
        self.assertEqual(reference["value_digest"], hashlib.sha256(value.encode()).hexdigest())
        owner = records[binding["owner_memory_id"]]
        self.assertEqual(owner["content_digest"], binding["owner_memory_digest"])
        self.assertEqual(owner["validity"], "active")
        if prior:
            self.assertEqual(records[bindings[prior["binding_id"]]["owner_memory_id"]]["validity"],
                             "superseded")
        self.report["binding_observations"].append({
            "boundary": "canonical_vault_binding",
            "reference": reference, "prior_reference": prior,
            "owner_memory_id": owner["id"], "source_entry_id": ingress["id"],
            "source_checksum": ingress["checksum"],
            "source_authority": source["authority"],
            "association_origin": binding["association_origin"],
            "vault_prefix_sha256": hashlib.sha256(complete).hexdigest(),
            "vault_event_count": len(events),
            "exact_binding_lineage": expected_lineage,
            "binding_event_sequences": [event["sequence"] for event in binding_events],
            "binding_event_digests": [event["digest"] for event in binding_events],
            "successful_owner_memory_ids": [receipt["evidence"]["id"] for receipt in receipts],
            "no_extra_binding_or_standalone_memory_mutations": True,
            "no_incomplete_mutation_tail": True,
        })
        return binding

    def test_capture_correction_context_dispatch_and_restart(self):
        self.report["journey_phase"] = "capture"
        initial_events, _ = self.canonical_vault(allow_missing=True)
        self.assertFalse([event for event in initial_events
                          if event["mutation"]["mutation"] == "binding_associated"],
                         "isolated profile already contained an operational binding")
        self.initial_vault_digests = [event["digest"] for event in initial_events]
        alias = "project-saffron-" + baseline.identity().lower()
        old_path = "status-" + baseline.identity().lower() + ".txt"
        current_path = "status-" + baseline.identity().lower() + ".txt"
        source_session = self.create_session("binding-source-" + baseline.identity())
        create_prompt = (
            f"Remember an exact operational mapping for {alias}: its status_path is {old_path}. "
            "Save this as a source-cited project-context memory with an exact workspace-path binding, "
            "using status_path as the property and the project name as its alias. "
            "The path is relative to this workspace. Use memory tools only; do not read any file yet.")
        _, _, entries, original_ingress = self.ask_with_entries(source_session, create_prompt)
        create = self.assert_write_recovery(entries, "memory_create", original_ingress)
        created = self.successful_json(create)
        self.assert_source_cited_mapping(create[0], original_ingress, old_path, alias=alias)
        original = created["binding"]
        self.assertEqual(original["key"]["property"], "status_path")
        self.canonical_binding(original, source_session, original_ingress, old_path,
                               receipts=[created], alias=alias)

        self.report["journey_phase"] = "correction"
        correct_prompt = (
            f"Correction for the same {alias} project: its status_path is now {current_path}. "
            f"The earlier {old_path} mapping is obsolete. Correct the existing source-cited memory "
            "and exact binding, keeping the same project identity and status_path property. "
            "Use memory tools only; do not read any file yet.")
        _, _, entries, correction_ingress = self.ask_with_entries(source_session, correct_prompt)
        correction = self.assert_write_recovery(
            entries, "memory_correct", correction_ingress,
            expected_owner=created["evidence"]["id"], expected_binding=original)
        corrected = self.successful_json(correction)
        correction_call = correction[0]
        self.assert_source_cited_mapping(correction_call, correction_ingress, current_path)
        self.assertEqual(correction_call["payload"]["arguments"]["expected_binding"], original)
        self.assertEqual(correction_call["payload"]["arguments"]["evidence_id"], created["evidence"]["id"])
        current = corrected["binding"]
        self.assertEqual(current["key"], original["key"])
        self.assertNotEqual(current["binding_id"], original["binding_id"])
        self.assertGreater(current["revision"], original["revision"])
        self.canonical_binding(current, source_session, correction_ingress, current_path, original,
                               receipts=[created, corrected], alias=alias)

        # Fixture bytes exist only in the real workspace filesystem. Neither path
        # nor nonce is supplied by the later task prompt or seeded into memory.
        (self.workspace / old_path).write_text("OBSOLETE-" + baseline.identity() + "\n")
        content = "CURRENT-" + baseline.identity() + "\n"
        (self.workspace / current_path).write_text(content)
        read_prompt = (
            f"For {alias}, look up its current exact status_path binding with memory_context and "
            "register it as required for a workspace read. Wait for that lookup's result before "
            "choosing the read path. Then use the read tool on that bound "
            "path and report the exact file contents. Use memory tools and one read only.")
        session = self.create_session("binding-current-read-" + baseline.identity())
        self.report["journey_phase"] = "current_read"
        snapshot, answer, entries, _ = self.ask_with_entries(session, read_prompt)
        self.assert_actual_read(session, snapshot, answer, entries, current_path, content, current)
        self.report["journey_phase"] = "restart"
        self.restart_runtime()
        self.canonical_binding(current, source_session, correction_ingress, current_path, original,
                               receipts=[created, corrected], alias=alias)
        content = "RESTARTED-" + baseline.identity() + "\n"
        (self.workspace / current_path).write_text(content)
        session = self.create_session("binding-restarted-read-" + baseline.identity())
        self.report["journey_phase"] = "restarted_read"
        snapshot, answer, entries, _ = self.ask_with_entries(session, read_prompt)
        self.assert_actual_read(session, snapshot, answer, entries, current_path, content, current)
        self.canonical_binding(current, source_session, correction_ingress, current_path, original,
                               receipts=[created, corrected], alias=alias)
        self.report["journey_phase"] = "complete"
        self.report["journey_completed"] = True

    @classmethod
    def restart_runtime(cls):
        before = {name: baseline.digest(cls.bin_root / name) for name in baseline.BINS}
        cls.stop_processes()
        exited = [process.returncode for process in cls.processes]
        cls.processes = []
        socket_path = cls.root / "agentd.sock"
        if socket_path.exists():
            socket_path.unlink()
        cls.launch(cls.daemon_argv, "daemon-restarted")
        cls.await_condition(lambda: socket_path.is_socket(), "restarted daemon socket", timeout=180)
        cls.launch(cls.web_argv, "web-restarted", {"KEITH_WEB_LOGIN_SECRET": cls.login_secret})
        cls.cookies = http.cookiejar.CookieJar()
        cls.http = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cls.cookies))
        cls.await_condition(cls.web_ready, "restarted Web listener", timeout=180)
        try:
            cls.http.open(urllib.request.Request(
                cls.origin + "/auth/session",
                data=urllib.parse.urlencode({"password": cls.login_secret}).encode(),
                headers={"Origin": cls.origin, "Content-Type": "application/x-www-form-urlencoded"},
            ), timeout=30).close()
        except urllib.error.HTTPError as error:
            if error.code not in (404, 503) or not list(cls.cookies):
                raise AssertionError("restarted Web login failed") from None
        cls.bootstrap = cls.get_json("/api/bootstrap")
        if cls.profile["id"] not in {profile["id"] for profile in cls.bootstrap["profiles"]}:
            raise AssertionError("profile identity changed on restart")
        after = {name: baseline.digest(cls.bin_root / name) for name in baseline.BINS}
        cls.report["restart_observations"].append({
            "stopped_process_exit_codes": exited,
            "launched_binary_hashes_unchanged": before == after,
            "profile_id": cls.profile["id"],
            "restart_kind": "daemon/Web stop and reopen against the same durable stores",
        })
        if before != after:
            raise AssertionError("restart changed launched binary identity")


def load_tests(_loader, _tests, _pattern):
    # No inherited baseline case can substitute for the required binding proof.
    suite = unittest.TestSuite()
    for number in range(1, 4):
        replicate = type(f"RuntimeBindingsReplicate{number}", (RuntimeBindings,),
                         {"replicate": f"replicate-{number}", "__module__": __name__})
        suite.addTest(replicate("test_capture_correction_context_dispatch_and_restart"))
    return suite


if __name__ == "__main__":
    unittest.main()
