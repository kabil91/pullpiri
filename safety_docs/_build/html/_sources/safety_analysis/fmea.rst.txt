.. ============================================================
   Pullpiri FMEA – Failure Mode and Effects Analysis
   One row per known failure mode across all 5 modules.
   ============================================================

Pullpiri Component FMEA
========================

This FMEA identifies failure modes in Pullpiri components that could
affect vehicle scenario lifecycle safety. Each row references the
existing mitigation mechanism traced to real Rust source code.

-----------------------------------------------------------------------------
APIServer Failure Modes
-----------------------------------------------------------------------------

.. comp_saf_fmea:: APIServer – etcd Write Fails Mid-Transaction
   :id: comp_saf_fmea__api__etcd_write_fail
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__api__yaml_validation
   :fault_id: comm_fault__etcd_write_partial
   :failure_effect: Scenario is written to etcd but Package write fails. Downstream
                    components query a Scenario with no matching Package and cannot
                    deploy the workload. System state is inconsistent.
   :mitigated_by: aou_req__pullpiri__etcd_replication
   :sufficient: yes
   :rationale: etcd Raft consensus atomicity on a 3-node quorum
               (aou_req__pullpiri__etcd_replication) rolls back any incomplete
               write on leader failure. No partial write persists.

   **Cause**: etcd leader crashes or network partitions between the Scenario
   write and the Package write in ``apply()``.

   **Detectability**: Automatic — etcd client returns an error on the failed
   write. The ``apply()`` function propagates the error to the REST caller (HTTP 500).

   **Source**: ``src/server/apiserver/src/artifact/mod.rs`` – ``apply()``

.. comp_saf_fmea:: APIServer – etcd Completely Unavailable at Startup
   :id: comp_saf_fmea__api__etcd_unavailable
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__api__connectivity_probe
   :fault_id: comm_fault__etcd_connection_refused
   :failure_effect: APIServer cannot read or write any configuration. All
                    scenario registration requests fail. The vehicle cannot
                    register new ADAS scenarios during the affected period.
   :mitigated_by: comp_req__api__connectivity_probe, aou_req__pullpiri__etcd_replication
   :sufficient: yes
   :rationale: The connectivity probe detects unavailability immediately and
               returns an error to the caller. The 3-node etcd quorum
               (aou_req__pullpiri__etcd_replication) ensures quorum is
               maintained unless more than one node fails simultaneously.

   **Cause**: All etcd nodes restart simultaneously, or the network between
   APIServer and etcd is partitioned.

   **Detectability**: Immediate — TCP connection probe returns false within
   3 seconds (bounded timeout in ``check_service_connectivity()``).
   HTTP 503 returned to caller.

   **Source**: ``src/server/apiserver/src/diagnostics.rs`` – ``check_service_connectivity()``


.. comp_saf_fmea:: APIServer – NodeAgent Unreachable but Command Sent
   :id: comp_saf_fmea__api__node_unreachable
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__api__node_probe
   :fault_id: comm_fault__grpc_timeout
   :failure_effect: If the APIServer sends a workload start command to an
                    unreachable NodeAgent, the gRPC call blocks until timeout
                    and returns an error. The workload is never started, but the
                    StateManager may already have advanced the scenario state
                    to ``running``, creating a state/reality mismatch.
   :mitigated_by: comp_req__api__node_probe, comp_req__sm__cluster_reconcile
   :sufficient: yes
   :rationale: comp_req__api__node_probe causes the APIServer to reject the
               dispatch before sending if the node is unreachable.
               comp_req__sm__cluster_reconcile causes the StateManager to
               detect the resulting degraded Package state and re-trigger
               ActionController to retry.

   **Source**: ``src/server/apiserver/src/diagnostics.rs`` – ``check_node_agent_connectivity()``

-----------------------------------------------------------------------------
FilterGateway Failure Modes
-----------------------------------------------------------------------------

.. comp_saf_fmea:: FilterGateway – DDS Message with Non-Numeric Value (Parse Failure)
   :id: comp_saf_fmea__fg__dds_parse_panic
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__fg__condition_eval
   :fault_id: sw_fault__dds_parse_error
   :failure_effect: A DDS message arrives where a numeric field contains a
                    non-numeric string (e.g. ``"N/A"`` instead of ``"35.0"``)
                    for a condition that uses ``lt/le/ge/gt``. The parse fails.
                    The scenario condition is never evaluated and the trigger
                    is missed, leaving the feature in WAITING state.
   :mitigated_by: comp_req__fg__condition_eval
   :sufficient: yes
   :rationale: ``meet_scenario_condition()`` uses Rust's ``parse::<f32>()``
               with explicit error mapping (``map_err``). A parse failure returns
               ``Err(...)`` rather than panicking. The error is logged and the
               loop continues — the scenario stays in WAITING, which is the
               safer option vs triggering on bad data.

   **Cause**: DDS publisher firmware bug or sensor hardware fault produces
   non-numeric data in a numeric field.

   **Detectability**: Immediate — ``map_err`` propagates the error, which is
   logged at level 3. The scenario stays in WAITING (observable via StateManager).

   **Source**: ``src/player/filtergateway/src/filter/mod.rs`` – ``meet_scenario_condition()``

.. comp_saf_fmea:: FilterGateway – DDS Feed Goes Silent (No Signal Received)
   :id: comp_saf_fmea__fg__dds_silence
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__fg__condition_eval
   :fault_id: comm_fault__dds_topic_silence
   :failure_effect: No DDS signal arrives. The FilterGateway waits indefinitely.
                    Active scenarios stall in WAITING state. The intended safety
                    action (e.g. Lane Assist activation) never triggers.
   :mitigated_by: comp_req__fg__dds_silence_detect, aou_req__pullpiri__dds_network
   :sufficient: yes
   :rationale: A 5-second ``tokio::time::timeout`` in ``process_dds_data()``
               detects silence and emits a ``[SAFETY]`` log event, enabling
               the external watchdog to escalate. DDS Liveliness QoS
               (aou_req__pullpiri__dds_network) provides a complementary
               network-layer silence notification. **Residual risk CLOSED.**

   **Cause**: DDS publisher process crashes, or network partition between
   the DDS bus and the FilterGateway host node.

   **Detectability**: Within 5 seconds — timeout fires and ``[SAFETY]`` log
   appears. External watchdog monitors for this log pattern.

   **Source**: ``src/player/filtergateway/src/manager.rs`` – ``process_dds_data()``


-----------------------------------------------------------------------------
StateManager Failure Modes
-----------------------------------------------------------------------------

.. comp_saf_fmea:: StateManager – Event Loop Deadlock / Freeze (No Heartbeat)
   :id: comp_saf_fmea__sm__heartbeat_missing
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__sm__heartbeat
   :fault_id: sw_fault__event_loop_deadlock
   :failure_effect: The StateManager gRPC receiver loop acquires a Mutex and
                    blocks indefinitely (deadlock with another task holding the
                    same lock, or a long-running handler). No state changes are
                    processed. All upstream components (APIServer, FilterGateway)
                    send state changes that are never acknowledged. Scenarios
                    silently stall with no error.
   :mitigated_by: comp_req__sm__heartbeat, aou_req__pullpiri__watchdog
   :sufficient: yes
   :rationale: comp_req__sm__heartbeat uses a shared ``AtomicU64`` counter that
               ``process_grpc_requests`` increments on every processed message.
               A separate heartbeat task reads this counter every 30 seconds
               and compares it to the previous value. If the counter has not
               advanced, the heartbeat emits a ``[HEARTBEAT] WARNING`` log at
               severity 5. On a multi-threaded Tokio runtime (``#[tokio::main]``
               default), the heartbeat task runs on its own worker thread and
               will correctly detect a deadlocked processing loop even though
               the deadlocked task is stuck. On a single-threaded runtime, the
               external OS watchdog (``aou_req__pullpiri__watchdog``) serves as
               the fallback — it detects the absence of any ``[HEARTBEAT]``
               log within a 60-second window.

   **Cause**: Mutex contention between tasks in ``process_grpc_requests``,
   a future that never completes, or a blocking synchronous call inside
   an async handler.

   **Detectability**: Within 30 seconds — heartbeat task detects stale counter
   and emits ``[HEARTBEAT] WARNING`` log. Within 60 seconds — external OS
   watchdog detects absence of ``[HEARTBEAT]`` log entirely.

   **Source**: ``src/player/statemanager/src/manager.rs`` – heartbeat task in ``run()``

.. comp_saf_fmea:: StateManager – gRPC Channel Drops Unexpectedly
   :id: comp_saf_fmea__sm__channel_close
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__sm__graceful_shutdown
   :fault_id: comm_fault__grpc_channel_close
   :failure_effect: If the upstream component (e.g. apiserver) crashes and
                    closes its channel, the StateManager's ``rx.recv()`` returns
                    ``None``. Without explicit handling, the processing loop
                    would spin indefinitely on a closed channel or panic.
   :mitigated_by: comp_req__sm__graceful_shutdown
   :sufficient: yes
   :rationale: comp_req__sm__graceful_shutdown mandates that the processing loop
               detects ``None`` from ``recv()`` and performs an orderly exit
               rather than looping or panicking. The channel-close handling in
               ``manager.rs`` implements this controlled degradation path.

   **Source**: ``src/player/statemanager/src/manager.rs`` – channel-close in ``run()``

-----------------------------------------------------------------------------
ActionController Failure Modes
-----------------------------------------------------------------------------

.. comp_saf_fmea:: ActionController – etcd Unavailable at Reconcile Time
   :id: comp_saf_fmea__ac__etcd_unavailable
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__ac__reconcile_do
   :fault_id: comm_fault__etcd_connection_lost
   :failure_effect: ``reconcile_do()`` calls ``common::etcd::get()`` to load the
                    Scenario and Package definitions. If etcd is unreachable at
                    this moment, the function returns an error immediately. The
                    workload is not started. The retry counter increments.
   :mitigated_by: comp_req__ac__retry_limit, aou_req__pullpiri__etcd_replication
   :sufficient: yes
   :rationale: The retry limit (comp_req__ac__retry_limit) caps the number of
               attempts at 3, preventing an infinite loop on persistent etcd
               outage. The 3-node etcd quorum (aou_req__pullpiri__etcd_replication)
               ensures quorum is maintained unless more than 1 node fails.
               A transient etcd blip resolves within one retry cycle.

   **Cause**: etcd network partition, quorum lost, or etcd process OOM-killed.

   **Detectability**: Immediate — ``etcd::get()`` returns ``Err`` which is
   propagated. The retry counter and ``[SAFETY]`` log after 3 failures make
   the failure visible.

   **Source**: ``src/player/actioncontroller/src/manager.rs`` – ``reconcile_do()``

.. comp_saf_fmea:: ActionController – Workload Continuously Crashes, Retry Limited
   :id: comp_saf_fmea__ac__infinite_retry
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__ac__reconcile_do
   :fault_id: sw_fault__infinite_restart_loop
   :failure_effect: A container with a persistent defect crashes immediately on
                    every start. Without a retry limit, the ActionController would
                    call ``start_workload()`` indefinitely, consuming CPU and
                    network resources without escalating to a permanent fault state.
   :mitigated_by: comp_req__ac__retry_limit, comp_req__na__backoff
   :sufficient: yes
   :rationale: comp_req__ac__retry_limit adds a ``MAX_RECONCILE_RETRIES = 3``
               per-scenario failure counter inside ``reconcile_do()``. After 3
               consecutive failures the function returns an error and logs a
               SAFETY escalation event — no further retries occur until the
               counter is manually reset. comp_req__na__backoff (NodeAgent
               exponential backoff) limits the restart rate at node level as
               a complementary measure. **Residual risk CLOSED — code fix applied.**

   **Source**: ``src/player/actioncontroller/src/manager.rs`` – ``reconcile_do()``

-----------------------------------------------------------------------------
NodeAgent Failure Modes
-----------------------------------------------------------------------------

.. comp_saf_fmea:: NodeAgent – gRPC Server Crashes While Container is Mid-Start
   :id: comp_saf_fmea__na__grpc_crash_mid_start
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__na__local_reconcile
   :fault_id: sw_fault__grpc_crash_partial_start
   :failure_effect: ActionController calls ``start_workload()`` via gRPC. The
                    NodeAgent gRPC server processes the request and calls Podman
                    to create the container, but then the NodeAgent gRPC server
                    crashes (OOM, panic) before sending the success response.
                    ActionController receives a gRPC error and treats it as a
                    failure (incrementing retry counter), but the container may
                    actually be running. Next reconcile attempt starts a duplicate.
   :mitigated_by: comp_req__na__local_reconcile
   :sufficient: partial
   :rationale: The reconciliation loop (comp_req__na__local_reconcile) runs every
               1 second and will detect the container in ``running`` state within
               1 second of the gRPC crash recovery. On the next ActionController
               reconcile, if ``start_workload()`` is called again, Podman will
               return an error for an already-running container, which is handled
               gracefully. **Partial — duplicate start attempt possible in the
               recovery window.**

   **Cause**: NodeAgent OOM kill or unhandled panic in the gRPC server thread
               during a concurrent start operation.

   **Detectability**: Within 1 second — reconciliation loop detects container
   state regardless of gRPC channel health.

   **Source**: ``src/agent/nodeagent/src/manager.rs`` – ``reconciliation_loop()``

.. comp_saf_fmea:: NodeAgent – Container Exits Unexpectedly
   :id: comp_saf_fmea__na__container_crash
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__na__local_reconcile
   :fault_id: sw_fault__container_exit
   :failure_effect: A running container process crashes (OOM kill, unhandled
                    panic, or kernel signal). The vehicle feature stops executing.
                    Without local detection, the StateManager would see a stale
                    ``Running`` status and take no recovery action.
   :mitigated_by: comp_req__na__local_reconcile, comp_req__na__backoff
   :sufficient: partial
   :rationale: The ``reconciliation_loop()`` detects ``exited``/``dead`` state
               within 1 second and calls Podman restart. ``calculate_backoff()``
               limits restart rate with exponential backoff capped at 300 s.
               **Partial — the reconciliation loop has no local retry cap
               (unlike ActionController which has MAX_RECONCILE_RETRIES = 3).
               The loop restarts indefinitely at the 300 s cap. Open action:
               add a local retry counter to NodeAgent reconcile loop, escalating
               to a permanent Error state after N consecutive failures.**

   **Cause**: Container application bug (unhandled panic), OOM kill by kernel,
   or kernel signal from an external process.

   **Detectability**: Within 1 second — ``get_list()`` from Podman shows
   ``exited`` or ``dead`` state on the next reconcile tick.

   **Open Action**: Add ``MAX_LOCAL_RETRIES`` to ``reconciliation_loop()``
   identical to ActionController pattern. Target: next safety sprint.

   **Source**: ``src/agent/nodeagent/src/manager.rs`` – ``reconciliation_loop()``

