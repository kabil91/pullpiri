.. ============================================================
   Pullpiri DFA – Dependent Failure Analysis
   Identifies shared resources that could cause multiple Pullpiri
   components to fail simultaneously (common-cause / Dependent
   Failure Initiators).
   ============================================================

Pullpiri Dependent Failure Analysis
=====================================

The DFA identifies **Dependent Failure Initiators (DFI)** — shared causes
that could simultaneously compromise multiple Pullpiri components, defeating
the Freedom from Interference (FFI) guarantee required by ISO 26262.

.. comp_saf_dfa:: Shared etcd Node – Disk Full Kills Both Write Path and Diagnostics WAL
   :id: comp_saf_dfa__pullpiri__etcd_full_disk
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__api__yaml_validation, comp_req__sm__cluster_reconcile
   :failure_id: dfi__common_cause__disk_exhaustion
   :failure_effect: If the disk hosting etcd fills up, two simultaneous failures
                    occur. The etcd Write-Ahead Log (WAL) cannot accept new
                    entries, causing all apiserver write operations to fail.
                    Simultaneously, the apiserver diagnostics module cannot write
                    log files, losing the diagnostic trail. Both the operational
                    path (scenario registration) and the safety evidence path
                    (diagnostics) fail together from a single cause.
   :mitigated_by: aou_req__pullpiri__etcd_replication
   :sufficient: yes
   :rationale: aou_req__pullpiri__etcd_replication requires a 3-node Raft
               cluster. In a properly configured multi-node etcd deployment,
               the WAL and data are distributed. Disk exhaustion on a single
               node causes that node to leave the quorum, but the remaining
               two nodes maintain quorum and continue serving writes.
               The integrating system must also configure etcd storage quotas
               and OS-level disk space monitoring as complementary measures.

   **Source**: ``src/server/apiserver/src/artifact/mod.rs`` – ``apply()``
   **Source**: ``src/server/apiserver/src/diagnostics.rs``

.. comp_saf_dfa:: Network Partition – FilterGateway and NodeAgent Both Lose Connectivity
   :id: comp_saf_dfa__pullpiri__network_partition
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__fg__condition_eval, comp_req__na__local_reconcile
   :failure_id: dfi__common_cause__network_partition
   :failure_effect: A network partition between the host node and the guest
                    nodes simultaneously isolates the FilterGateway from the
                    DDS vehicle signal bus AND isolates the NodeAgent from the
                    ActionController. Scenarios stall in WAITING (FilterGateway
                    sees no DDS signals) while containers may be running in an
                    unmonitored state (NodeAgent reports no status to StateManager).
                    Both independent safety paths are disabled by the same fault.
   :mitigated_by: aou_req__pullpiri__watchdog, aou_req__pullpiri__dds_network
   :sufficient: yes
   :rationale: aou_req__pullpiri__watchdog ensures the OS-level watchdog detects
               that Pullpiri processes are no longer communicating (missed
               keepalive) and triggers a system-level safe state transition.
               aou_req__pullpiri__dds_network requires DDS Liveliness QoS so
               the FilterGateway receives a network-loss notification rather
               than silently stalling. Together these ensure the vehicle
               transitions to a defined safe state rather than remaining in
               an undefined partitioned state.

   **Source**: ``src/player/filtergateway/src/filter/mod.rs``
   **Source**: ``src/agent/nodeagent/src/manager.rs``

.. comp_saf_dfa:: Podman Daemon Crash – NodeAgent Loses Container Lifecycle Control
   :id: comp_saf_dfa__pullpiri__podman_daemon_crash
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__na__local_reconcile, comp_req__na__backoff
   :failure_id: dfi__common_cause__podman_daemon_failure
   :failure_effect: If the Podman daemon process crashes or becomes unresponsive,
                    all NodeAgent container lifecycle API calls fail.
                    NodeAgent reconciliation enters continuous error mode:
                    ``get_list()`` returns ``Err`` and ``handle_missing_container``
                    attempts all fail. Without the Podman daemon, NodeAgent cannot
                    recover crashed workload containers, violating SG_002.
   :mitigated_by: aou_req__pullpiri__watchdog, aou_req__pullpiri__podman_version
   :sufficient: yes
   :rationale: Integrator shall configure Podman as a systemd service with
               ``Restart=on-failure`` and ``RestartSec=5s``. This ensures
               automatic restart of the Podman daemon within 5 seconds of a
               crash. ``aou_req__pullpiri__watchdog`` covers the case where
               Podman restart itself fails — the OS watchdog detects missed
               NodeAgent keepalives and triggers a system-level safe state.

   **Integrator Verification Steps**:

   1. Confirm ``podman.service``/``podman.socket`` has ``Restart=on-failure``.
   2. Run ``systemctl kill podman``; verify Podman restarts within 10 s.
   3. Confirm NodeAgent resumes ``[Reconciliation]`` log entries after restart.
   4. Confirm OS watchdog fires if Podman does not recover within 60 s.

   **Source**: ``src/agent/nodeagent/src/manager.rs`` – ``reconciliation_loop()``

.. comp_saf_dfa:: Host OS Network Fault – gRPC and DDS Simultaneously Disconnected
   :id: comp_saf_dfa__pullpiri__host_network_fault
   :status: valid
   :safety_level: ASIL_B
   :violates: comp_req__fg__dds_silence_detect, comp_req__sm__heartbeat,
              comp_req__ac__reconcile_do, comp_req__na__local_reconcile
   :failure_id: dfi__common_cause__host_network_failure
   :failure_effect: A host OS network interface failure simultaneously
                    disconnects DDS (FilterGateway stops receiving vehicle
                    signals) and all gRPC channels (SM, AC, NA lose
                    inter-module communication). All four modules are degraded
                    by the same single fault: scenarios stall in WAITING,
                    container reconciliation cannot report status, and the
                    heartbeat log cannot be written. This defeats FFI.
   :mitigated_by: aou_req__pullpiri__watchdog, aou_req__pullpiri__dds_network
   :sufficient: yes
   :rationale: Integrator shall configure the OS-level network watchdog to
               monitor the Pullpiri host network interface. When the interface
               goes down, systemd ``WatchdogSec=60`` detects absence of
               inter-process keepalives and triggers a system-level safe
               state transition within the 60-second ASIL-B reaction window.
               ``aou_req__pullpiri__dds_network`` ensures DDS Liveliness QoS
               signals network loss to FilterGateway, allowing orderly shutdown.

   **Integrator Verification Steps**:

   1. Configure ``WatchdogSec=60`` for all 5 Pullpiri systemd services.
   2. Simulate network loss (``ip link set <iface> down``); verify all 5 services
      report watchdog keepalive failure within 60 s.
   3. Verify system transitions to defined safe state within 60 s reaction window.
   4. Restore network (``ip link set <iface> up``); verify all 5 services resume.

   **Source**: ``src/player/filtergateway/src/manager.rs`` – DDS silence detection
   **Source**: ``src/player/statemanager/src/manager.rs`` – heartbeat probe
