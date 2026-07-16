.. ============================================================
   Pullpiri Component Requirements
   All items below are traced via // req-traceability: comments
   in the actual Rust source files. The S-CORE source code linker
   scans those files and creates machine-verified live links.
   ============================================================

Pullpiri Component Requirements
=================================

Pullpiri targets **ASIL-B (software)**. Every requirement below is traced
to a ``// req-traceability:`` comment placed directly above the implementing
Rust function. The S-CORE source linker verifies these links at build time.

-----------------------------------------------------------------------------
APIServer Component
-----------------------------------------------------------------------------

.. comp_req:: APIServer – YAML Artifact Validation Before DB Write
   :id: comp_req__api__yaml_validation
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__etcd_replication
   :rationale: Without schema validation, a malformed or truncated YAML payload
               can be written to etcd, corrupting scenario state for all downstream
               components. This is classified as an Input Validation safety mechanism
               per ISO 26262 Part 6 §7.4.4.

   The APIServer ``apply()`` function shall split, parse, and verify that the
   incoming YAML payload contains at least one valid ``Scenario`` document and
   one valid ``Package`` document before initiating any etcd write operation.

   **Source**: ``src/server/apiserver/src/artifact/mod.rs`` – ``apply()``

.. comp_req:: APIServer – TCP Connectivity Probe with Bounded Timeout
   :id: comp_req__api__connectivity_probe
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__watchdog
   :rationale: Unbounded blocking on network I/O in a safety-relevant orchestrator
               can cause the entire service to deadlock. The 3-second timeout bounds
               the diagnostic probe and prevents thread starvation (ISO 26262
               Part 5 §8.4 – Diagnostic Coverage).

   The APIServer ``check_service_connectivity()`` function shall use a
   ``tokio::time::timeout`` of exactly 3 seconds when probing downstream
   service endpoints via TCP connect.

   **Source**: ``src/server/apiserver/src/diagnostics.rs`` – ``check_service_connectivity()``

.. comp_req:: APIServer – NodeAgent Reachability Probe
   :id: comp_req__api__node_probe
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__watchdog
   :rationale: Before routing workload commands to a NodeAgent, the APIServer
               must verify the agent is reachable. Sending a workload start
               command to an unreachable node results in a silent failure with
               no recovery path.

   The APIServer ``check_node_agent_connectivity()`` function shall probe the
   NodeAgent TCP port (47004) before dispatching any workload operation.

   **Source**: ``src/server/apiserver/src/diagnostics.rs`` – ``check_node_agent_connectivity()``

-----------------------------------------------------------------------------
StateManager Component
-----------------------------------------------------------------------------

.. comp_req:: StateManager – State Transition Plausibility Check
   :id: comp_req__sm__validate_state
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__posix_os
   :rationale: State transitions with empty resource names, empty transition IDs,
               or identical current/target states indicate malformed or duplicate
               requests. Accepting such inputs would corrupt the state machine
               and cause erroneous scenario lifecycle decisions (ISO 26262
               Part 6 §7.4.6 – Plausibility Check).

   The StateManager ``validate_state_change()`` function shall reject any
   StateChange request where the resource name, transition ID, or source field
   is empty, or where the current state equals the target state.

   **Source**: ``src/player/statemanager/src/state_machine.rs`` – ``validate_state_change()``

.. comp_req:: StateManager – 30-Second Heartbeat Alive Signal
   :id: comp_req__sm__heartbeat
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__watchdog
   :rationale: Without a periodic alive signal, a deadlocked or frozen StateManager
               event loop is undetectable from outside. An external watchdog that only
               monitors process existence (PID alive) will not detect a live-locked
               thread that has stopped processing messages. ISO 26262 Part 6 §7.4.9
               requires a watchdog trigger to verify that safety-relevant processing
               has not silently frozen.

   The StateManager ``run()`` function shall emit a log message tagged
   ``[HEARTBEAT]`` every 30 seconds while the event loop is operational.
   The external watchdog shall flag a safety event if no heartbeat is observed
   within a 60-second window (2× the emission interval).

   **Source**: ``src/player/statemanager/src/manager.rs`` – heartbeat task in ``run()``

   :id: comp_req__sm__cluster_reconcile
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__etcd_replication
   :rationale: When a package enters an error/dead state, the StateManager must
               actively trigger ActionController to attempt recovery rather than
               leaving the cluster in a silent error state (ISO 26262
               Part 6 §7.4.12 – Fault Reaction).

   The StateManager shall invoke ``trigger_action_controller_reconcile_internal()``
   via gRPC when any Package enters a degraded or error state, requesting that
   ActionController attempt recovery to the desired running state.

   **Source**: ``src/player/statemanager/src/manager.rs`` – ``trigger_action_controller_reconcile_internal()``

.. comp_req:: StateManager – Graceful Shutdown on Channel Close
   :id: comp_req__sm__graceful_shutdown
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__posix_os
   :rationale: When an upstream component closes its channel (e.g. apiserver
               shuts down), the StateManager must detect this and exit its
               processing loop cleanly without leaving dangling threads or
               unclosed file descriptors (ISO 26262 Part 6 §7.4.11 –
               Controlled Degradation).

   The StateManager container and state-change processing loops shall detect
   channel closure (``None`` from ``recv()``) and perform an orderly exit,
   logging a controlled degradation event rather than panicking.

   **Source**: ``src/player/statemanager/src/manager.rs`` – channel-close handling in ``run()``

-----------------------------------------------------------------------------
ActionController Component
-----------------------------------------------------------------------------

.. comp_req:: ActionController – Self-Healing Reconcile Loop
   :id: comp_req__ac__reconcile_do
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__posix_os
   :rationale: When the StateManager detects a Package failure and triggers
               reconciliation, the ActionController must compare actual versus
               desired container state and take corrective action. Failure to
               act would leave the system in a permanent degraded state
               (ISO 26262 Part 6 §7.4.12 – Fault Recovery).

   The ActionController ``reconcile_do()`` function shall compare the current
   and desired status of a scenario's workloads. If the desired state is
   ``Running`` and the current state differs, it shall invoke ``start_workload()``
   for each model in the scenario's package.

   **Source**: ``src/player/actioncontroller/src/manager.rs`` – ``reconcile_do()``

.. comp_req:: ActionController – Retry Limit on Failing Workloads
   :id: comp_req__ac__retry_limit
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__watchdog
   :rationale: Without a hard retry cap, a continuously crashing workload would
               cause the ActionController to enter an infinite restart loop,
               saturating host CPU and starving other safety-critical processes
               (ISO 26262 Part 6 §7.4.12 – Fault Reaction).

   The ActionController shall limit the number of consecutive reconcile attempts
   per scenario to ``MAX_RECONCILE_RETRIES`` (default: 3). Once the limit is
   reached, the scenario shall be escalated to a permanent error state and a
   ``[SAFETY]`` log event emitted. No further restart attempts shall occur
   without manual operator intervention.

   **Source**: ``src/player/actioncontroller/src/manager.rs`` – ``MAX_RECONCILE_RETRIES`` constant and retry_counts check

-----------------------------------------------------------------------------
FilterGateway Component
-----------------------------------------------------------------------------

.. comp_req:: FilterGateway – DDS Signal Condition Evaluation
   :id: comp_req__fg__condition_eval
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__dds_network
   :rationale: The FilterGateway is the component that converts raw vehicle DDS
               signals into scenario trigger decisions. Incorrect condition
               evaluation would cause scenarios to activate on wrong signals
               (false positive) or fail to activate on correct signals
               (false negative) — both represent safety-relevant failures.

   The FilterGateway ``meet_scenario_condition()`` function shall evaluate the
   DDS data field value against the configured scenario condition expression
   (``eq``, ``lt``, ``le``, ``ge``, ``gt``) and shall only trigger the
   ActionController when the condition is precisely satisfied.

   **Source**: ``src/player/filtergateway/src/filter/mod.rs`` – ``meet_scenario_condition()``

.. comp_req:: FilterGateway – DDS Silence Detection with 5-Second Timeout
   :id: comp_req__fg__dds_silence_detect
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__dds_network
   :rationale: If the DDS vehicle data feed becomes silent (due to publisher
               crash, network partition, or middleware failure), all scenario
               conditions will never evaluate and active features will stall
               indefinitely. A bounded silence detector is required to escalate
               this failure within the ASIL-B latency requirement
               (ISO 26262 Part 6 §7.4.9 – Watchdog Pattern).

   The FilterGateway ``process_dds_data()`` function shall monitor the DDS
   message stream and detect silence (no messages received) within a configurable
   timeout window (default: ``DDS_SILENCE_TIMEOUT_SECS = 5``). Upon timeout, a
   ``[SAFETY]`` log event shall be emitted. The external watchdog can then
   escalate within ASIL-B latency bounds.

   **Source**: ``src/player/filtergateway/src/manager.rs`` – ``process_dds_data()`` timeout branch

-----------------------------------------------------------------------------
NodeAgent Component
-----------------------------------------------------------------------------

.. comp_req:: NodeAgent – Local Container Reconciliation Loop
   :id: comp_req__na__local_reconcile
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__watchdog
   :rationale: A workload container that exits unexpectedly (crash, OOM kill)
               must be detected and restarted locally without requiring a full
               round-trip through the StateManager and ActionController
               (ISO 26262 Part 6 §7.4.9 – Watchdog Pattern).

   The NodeAgent ``reconciliation_loop()`` shall run continuously, comparing
   desired container states against actual Podman container states every 1
   second, and invoking restart or re-creation procedures on any discrepancy.

   **Source**: ``src/agent/nodeagent/src/manager.rs`` – ``reconciliation_loop()``

.. comp_req:: NodeAgent – Exponential Backoff on Container Restart
   :id: comp_req__na__backoff
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :satisfies: aou_req__pullpiri__watchdog
   :rationale: Restarting a repeatedly crashing container at maximum rate (tight
               loop) can exhaust CPU and I/O resources, starving other services
               and introducing a Dependent Failure Initiator (DFI). Exponential
               backoff limits the blast radius of a persistent container failure
               (ISO 26262 Part 6 §7.4.9).

   The NodeAgent ``calculate_backoff()`` function shall compute restart delay
   as ``min(10 × 2^restart_count, 300)`` seconds, capping the maximum wait at
   300 seconds regardless of the number of prior restarts.

   **Source**: ``src/agent/nodeagent/src/manager.rs`` – ``calculate_backoff()``

.. comp_req:: APIServer – YAML Schema Validation for Settings Service
   :id: comp_req__api__schema_validation
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: YES
   :satisfies: aou_req__pullpiri__mtls_grpc
   :rationale: The Settings Service in the APIServer processes configuration
               updates that affect Pullpiri module behaviour. Without schema
               validation, a malformed or malicious settings payload could
               corrupt module configuration, violating SG_003 (Input Validation)
               and SG_005 (Fault Reaction Bound).

   The APIServer Settings Service ``validate()`` function shall verify that
   all received configuration payloads conform to the expected settings
   schema before applying them. Invalid payloads shall be rejected with
   an appropriate error response and logged.

   **Source**: ``src/server/apiserver/src/`` – Settings Service ``validate()``

.. comp_req:: APIServer – YAML Artifact Signature Verification
   :id: comp_req__api__yaml_signing
   :reqtype: Functional
   :status: valid
   :safety: ASIL_B
   :security: YES
   :satisfies: aou_req__pullpiri__mtls_grpc
   :rationale: The APIServer ``apply()`` function accepts YAML payloads
               containing Scenario and Package specifications that directly
               control workload container lifecycle. Without signature
               verification, a tampered or forged YAML payload could
               activate unauthorised scenarios, violating SG_003 and SG_006.

   The APIServer ``apply()`` function shall verify a digital signature or
   HMAC over each received YAML payload before parsing or storing it.
   Payloads with missing, invalid, or expired signatures shall be rejected
   with a 403 Forbidden response and logged as a security event.

   **Source**: ``src/server/apiserver/src/artifact/mod.rs`` – ``apply()``
   **Note**: Implementation of cryptographic signing mechanism is P2 (pending).
