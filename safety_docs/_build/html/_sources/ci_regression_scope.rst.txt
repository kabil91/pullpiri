.. ============================================================
   Pullpiri CI Regression Scope
   Documents which safety-tagged source files trigger which
   test suites in the CI pipeline, and the required build gates
   for ASIL-B compliance (ISO 26262 Part 6 §7.4.7).
   ============================================================

Pullpiri CI Regression Scope
==============================

Pullpiri uses a **docs-as-code** CI pipeline where every pull request that
touches a safety-tagged source file must pass the corresponding test suite
AND rebuild the Sphinx-Needs HTML safety evidence before merge is permitted.

Source File → Test Suite Mapping
----------------------------------

The following matrix lists every safety-tagged source file, its corresponding
test suite, the test count, and the ASIL level:

=======================================================  =============================================  ===========  ==========
Source File (safety-tagged)                              Test Suite                                     Test Count   ASIL Level
=======================================================  =============================================  ===========  ==========
``src/server/apiserver/src/artifact/mod.rs``             ``tests/integration/api_integration.rs``       9 tests      ASIL-B
``src/server/apiserver/src/diagnostics.rs``              ``tests/integration/api_integration.rs``       9 tests      ASIL-B
``src/player/filtergateway/src/filter/mod.rs``           ``tests/integration/filter_integration.rs``    23 tests     ASIL-B
``src/player/filtergateway/src/manager.rs``              ``tests/integration/fg_manager_integration``   9 tests      ASIL-B
``src/player/statemanager/src/manager.rs``               inline ``#[test]`` blocks                      15+ tests    ASIL-B
``src/player/actioncontroller/src/manager.rs``           inline ``#[test]`` blocks                      9+ tests     ASIL-B
``src/agent/nodeagent/src/manager.rs``                   inline ``#[test]`` blocks                      6+ tests     ASIL-B
``safety_docs/docs/requirements/comp_req.rst``           Sphinx-Needs build (``sphinx-build -W``)       0 err/warn   ASIL-B
``safety_docs/docs/aou/aou_req.rst``                     Sphinx-Needs build                             0 err/warn   ASIL-B
``safety_docs/docs/safety_analysis/fmea.rst``            Sphinx-Needs build                             0 err/warn   ASIL-B
``safety_docs/docs/safety_analysis/dfa.rst``             Sphinx-Needs build                             0 err/warn   ASIL-B
=======================================================  =============================================  ===========  ==========

Required CI Build Gates (ASIL-B)
----------------------------------

All of the following gates **must pass** before any PR touching a
safety-tagged file is merged:

1. **``cargo build --workspace``** — No compilation errors.
2. **``cargo test --workspace``** — All unit and inline integration tests pass.
3. **``cargo clippy --workspace -- -D warnings``** — Zero clippy warnings.
4. **``sphinx-build -W -b html safety_docs/docs safety_docs/_build/html``** —
   Zero errors and zero warnings. Validates all ``// req-traceability:`` tags,
   ``comp_req__*`` nodes, ``aou_req__*`` references, and DFA linkage.

Traceability Tag Verification Command
---------------------------------------

To verify all 15+ ``req-traceability`` tags locally::

   grep -rn "req-traceability:" src/ --include="*.rs" | sort

Expected IDs (minimum):

- ``comp_req__api__yaml_validation``
- ``comp_req__api__schema_validation``
- ``comp_req__api__yaml_signing``
- ``comp_req__api__connectivity_probe``
- ``comp_req__api__node_probe``
- ``comp_req__fg__condition_eval``
- ``comp_req__fg__dds_silence_detect``
- ``comp_req__sm__validate_state``
- ``comp_req__sm__heartbeat``
- ``comp_req__sm__cluster_reconcile``
- ``comp_req__sm__graceful_shutdown`` (x2)
- ``comp_req__ac__reconcile_do``
- ``comp_req__ac__retry_limit``
- ``comp_req__na__backoff``
- ``comp_req__na__local_reconcile`` (x2)
