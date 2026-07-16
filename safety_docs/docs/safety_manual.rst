.. ============================================================
   Pullpiri Safety Manual
   Integrator-facing document for SEooC deployment.
   ============================================================

Pullpiri Safety Manual
=======================

.. doc:: Pullpiri Safety Manual
   :id: doc__pullpiri_safety_manual
   :status: valid
   :safety: ASIL_B
   :security: YES
   :realizes: comp__pullpiri_core
   :rationale: This manual documents integrator obligations and residual risks
               for Pullpiri. Without this document, system integrators cannot
               verify that they have satisfied all Assumptions of Use required
               for Pullpiri's ASIL-B software safety claim to hold.

   This safety manual describes the safe integration rules, Assumptions of Use
   (AoU), and residual risks for **Pullpiri**, the S-CORE vehicle workload
   orchestration platform. Pullpiri is developed as a
   **Safety Element out of Context (SEooC)** at **ASIL-B (software)**.

Integrator Obligations
-----------------------

The vehicle platform integrator **must** satisfy all items below before
deploying Pullpiri in an ASIL-B context:

1. **Qualified OS** – Provide a POSIX-qualified OS with deterministic
   real-time scheduling.
   See ``aou_req__pullpiri__posix_os``.

2. **etcd 3-node Quorum** – Deploy etcd as a minimum 3-node Raft cluster.
   Single-node etcd is **not permitted** for ASIL-B use.
   See ``aou_req__pullpiri__etcd_replication``.

3. **mTLS on all gRPC channels** – Configure mutual TLS on all Pullpiri
   service endpoints at deployment time.
   See ``aou_req__pullpiri__mtls_grpc``.

4. **cgroup Isolation for apiserver** – Enforce CPUQuota ≤ 50% and
   MemoryMax ≤ 128 MiB on the apiserver process.
   See ``aou_req__pullpiri__cgroup_isolation``.

5. **External Process Watchdog** – Provide an OS-level watchdog that
   monitors all Pullpiri processes and triggers safe-state on missed
   keepalive signals.
   See ``aou_req__pullpiri__watchdog``.

6. **DDS Liveliness QoS** – Configure DDS Liveliness QoS on all vehicle
   data topics consumed by FilterGateway.
   See ``aou_req__pullpiri__dds_network``.

Residual Risks
--------------

The following risks are documented and accepted pending future code changes:

- **ActionController infinite retry** (``comp_saf_fmea__ac__infinite_retry``):
  The ActionController has no retry counter or escalation path when a container
  keeps crashing. Mitigated partially by NodeAgent exponential backoff
  (``comp_req__na__backoff``). Full mitigation requires a retry counter
  and Error state escalation in ``reconcile_do()``. Accepted at ASIL-B
  pending implementation.

- **FilterGateway DDS silence** (``comp_saf_fmea__fg__dds_silence``):
  No in-code silence detector. Mitigated entirely by DDS Liveliness QoS
  (``aou_req__pullpiri__dds_network``). Accepted at ASIL-B as an integrator
  obligation.

ASIL-B Software Safety Claim
------------------------------

Subject to all Assumptions of Use being satisfied by the integrating system,
Pullpiri makes the following ASIL-B software safety claim:

   *Pullpiri guarantees that vehicle workload scenarios are registered,
   activated, monitored, and reconciled in a deterministic, traceable manner.
   Input configurations are validated before database writes. State transitions
   are plausibility-checked before execution. Failed containers are detected
   and restarted with bounded backoff. All safety mechanisms are traceable
   to specific Rust functions via machine-verified source code links.*
