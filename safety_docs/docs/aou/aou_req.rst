.. ============================================================
   Pullpiri Assumptions of Use (AoU)
   Pullpiri is developed as a SEooC (Safety Element out of Context).
   The vehicle platform integrator MUST fulfil these assumptions
   for Pullpiri's safety claims to hold.
   ============================================================

Pullpiri Assumptions of Use
============================

Pullpiri is developed as a **Safety Element out of Context (SEooC)** targeting
**ASIL-B (software)**. The integrating vehicle platform must fulfil every
assumption below for Pullpiri's safety properties to be valid.

.. aou_req:: POSIX-Qualified OS Required
   :id: aou_req__pullpiri__posix_os
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Pullpiri processes rely on POSIX thread scheduling and signal
               semantics. Without a qualified OS layer, scheduling determinism
               and process isolation guarantees are invalidated.

   The integrating system shall provide a POSIX-qualified operating system
   (e.g. QNX SDP or a Linux kernel with an RT patch qualified to ASIL-B) that
   guarantees deterministic thread scheduling and process isolation.

.. aou_req:: etcd Consensus Cluster Required
   :id: aou_req__pullpiri__etcd_replication
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Pullpiri stores all scenario, package, and model configuration
               in etcd. A single-node etcd instance has no fault tolerance.
               A 3-node quorum ensures atomic write guarantees (Raft consensus)
               so a mid-write crash cannot leave partially written configuration.

   The integrating system shall deploy etcd as a minimum 3-node Raft quorum
   cluster. Single-node etcd deployments are not permitted for ASIL-B use.

.. aou_req:: mTLS on all gRPC Channels
   :id: aou_req__pullpiri__mtls_grpc
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: YES
   :rationale: Pullpiri gRPC endpoints (apiserver, statemanager, actioncontroller,
               filtergateway, nodeagent) accept unauthenticated connections by
               default. Without mTLS, any vehicle network participant can inject
               commands or corrupt state.

   The integrating system shall configure mutual TLS (mTLS) on all Pullpiri
   gRPC and REST endpoints at deployment time via sidecar proxy or platform
   network policy.

.. aou_req:: OS cgroup Resource Isolation for apiserver
   :id: aou_req__pullpiri__cgroup_isolation
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: The apiserver accepts external REST requests. Without CPU and
               memory limits, a malformed or malicious payload could exhaust
               system resources, starving safety-critical services (Freedom
               from Interference violation).

   The integrating system shall enforce Linux cgroup v2 (or equivalent OS
   partitioner) limits on the apiserver process: CPUQuota ≤ 50% and
   MemoryMax ≤ 128 MiB.

.. aou_req:: External Process Watchdog Required
   :id: aou_req__pullpiri__watchdog
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Pullpiri components do not implement an internal hardware
               watchdog. Without an external supervisor, a deadlocked or
               crashed process will not be detected and restarted within the
               ASIL-B reaction time budget.

   The integrating system shall provide an OS-level watchdog (e.g. systemd
   watchdog with WatchdogSec=, QNX SLM, or equivalent) that monitors all
   Pullpiri service processes and triggers restart or safe-state transition
   on missed keepalive signals.

.. aou_req:: DDS Network Liveliness QoS Required
   :id: aou_req__pullpiri__dds_network
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: The FilterGateway subscribes to vehicle DDS topics. If the
               DDS publisher goes silent without a liveliness event, the
               FilterGateway will wait indefinitely for a signal that will
               never arrive, causing the scenario to stall in WAITING state.

   The integrating DDS network shall configure Liveliness QoS on all
   vehicle data topics consumed by Pullpiri FilterGateway so that topic
   silence is detected and reported within the application's worst-case
   reaction time.

.. aou_req:: Podman Runtime Version ≥ 4.x Required
   :id: aou_req__pullpiri__podman_version
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Pullpiri NodeAgent uses the Podman v4 libpod REST API
               (``/v4.0.0/libpod/containers/{id}/restart``) for container
               lifecycle management. Older Podman versions may not expose this
               endpoint or may have incompatible restart semantics, invalidating
               the container recovery guarantee required by SG_002.

   The integrating system shall deploy Podman version ≥ 4.0.0. NodeAgent
   container lifecycle operations (start, restart, inspect, list) are
   verified only against the Podman v4 libpod REST API.

.. aou_req:: No Exceptions Policy for Safety Modules
   :id: aou_req__pullpiri__no_exceptions
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Pullpiri is implemented in Rust, which uses panics instead of
               exceptions. All Pullpiri modules must never silence panics
               (no ``catch_unwind`` in safety paths). A panic must propagate
               and terminate the process so the OS watchdog can detect and
               restart the module within the ASIL-B reaction window.

   The integrating system shall NOT wrap any Pullpiri module in an exception
   catcher or panic suppressor. Panics must terminate the Pullpiri process
   so that ``aou_req__pullpiri__watchdog`` can trigger recovery.

.. aou_req:: Error Reaction – Safety Apps Must Handle All Return Values
   :id: aou_req__pullpiri__error_reaction
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Safety applications that send commands to Pullpiri modules
               must always inspect the ``Result<T, E>`` or gRPC status code
               returned by Pullpiri APIs. Silently ignoring errors from
               safety-critical calls violates ISO 26262 §7.4.4 (error detection).

   External safety applications integrated with Pullpiri SHALL read and
   react to all return values from Pullpiri gRPC and REST APIs. Errors must
   trigger application-level safe-state actions within the system's worst-case
   reaction time budget.

.. aou_req:: Flow Monitoring – Alive Signal Required From All Modules
   :id: aou_req__pullpiri__flow_monitoring
   :reqtype: Non-Functional
   :status: valid
   :safety: ASIL_B
   :security: NO
   :rationale: Pullpiri StateManager implements an AtomicU64 heartbeat counter
               (comp_req__sm__heartbeat) that advances on every processed gRPC
               message. The OS watchdog monitors the ``[HEARTBEAT]`` log line
               within a 60-second window. The same alive-signal pattern must be
               applied to all five modules (APIServer, FilterGateway,
               ActionController, NodeAgent) to achieve end-to-end program flow
               monitoring per ISO 26262 §7.4.9.

   The integrating system shall configure per-process watchdog monitoring for
   all five Pullpiri modules. Each module's alive signal must be verified
   within a 60-second window. If any module's alive signal is absent, the
   system shall transition to a defined safe state.

