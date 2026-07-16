Tool Qualification Summary
===========================

.. note::

   ISO 26262 Part 8 §11 requires that every software tool used in a safety-relevant
   activity be assigned a Tool Confidence Level (TCL). This document records the TCL
   classification, version pinning, and qualification basis for every tool used in the
   Pullpiri development and verification process.

   **Tool Confidence Level definitions:**

   - **TCL-1**: Tool failure is not safety-relevant (errors caught by other means).
   - **TCL-2**: Tool failure could introduce undetected errors in safety evidence.
   - **TCL-3**: Tool failure directly affects the correctness of safety-critical output
     without any independent verification.

.. list-table:: Tool Qualification Summary
   :header-rows: 1
   :widths: 20 15 10 15 40

   * - Tool
     - Version (Pinned)
     - TCL
     - ISO 26262 Clause
     - Qualification Basis
   * - ``rustc`` (Rust compiler)
     - 1.92.0 (ded5c06cf 2025-12-08)
     - TCL-2
     - Part 6 §8 (Implementation)
     - Rust compiler test suite (``rustc-tests``); MSRV pinned via ``rust-toolchain.toml``;
       output validated by the cargo-test framework. Compiler errors and UB are caught by
       Miri and sanitizers. Well-established industrial compiler (10+ years of production use).
   * - ``cargo test``
     - 1.92.0 (bundled with rustc)
     - TCL-2
     - Part 6 §9 (Unit Testing)
     - Test runner is part of the Rust standard toolchain; test results are serialised
       to JUnit XML by ``cargo2junit`` for audit trail. Failures are deterministic.
   * - ``cargo-tarpaulin``
     - 0.32.3
     - TCL-2
     - Part 6 §9.4.5 (Coverage)
     - Tarpaulin injects instrumentation at the LLVM IR level; its own test suite
       validates coverage counting accuracy. Coverage outputs (lcov, Cobertura XML,
       HTML) are compared against manually-counted reference values on sample modules.
       Threshold gate (``--fail-under 70``) provides independent verification.
   * - ``sphinx-build`` with ``sphinx-needs``
     - Sphinx 9.1.0 / sphinx-needs 8.1.1
     - TCL-2
     - Part 6 §7.2.11 (Traceability)
     - ``sphinx-build -W`` fails on any unresolved requirement ID, providing automated
       verification of every ``req-traceability`` tag. The sphinx-needs project has its
       own integration test suite; version pinned in ``safety_docs/requirements.txt``.
   * - ``cargo clippy``
     - 1.92.0 (bundled with rustc)
     - TCL-1
     - Part 6 §9.4.9 (Static Analysis)
     - Clippy is a well-known Rust linting tool used across automotive and embedded Rust
       projects. Findings are cross-checked manually during code review. Tool errors would
       produce false negatives (missed warnings), not false safety guarantees.
   * - ``cargo deny``
     - Latest stable (pinned in CI)
     - TCL-1
     - Part 6 §9.4.9 (Dependency Security)
     - Checks against the RustSec advisory database for known CVEs and license violations.
       False negatives (missed CVEs) are tolerable since dependency versions are also
       reviewed during peer code review and supply-chain scans.
   * - ``cargo-fuzz`` / ``ThreadSanitizer``
     - nightly (pinned to Rust 1.92+)
     - TCL-2
     - Part 6 §9.4.9 (Dynamic Analysis)
     - TSan detects data races at runtime; its output is an independent check on the
       safety of the ``Arc<Mutex<>>`` and ``Arc<AtomicU64>`` patterns. TSan errors
       cause non-zero exit codes; CI blocks merge on any detected race.
   * - ``CodeQL``
     - 2.x (github/codeql-action@v3)
     - TCL-1
     - Part 6 §9.4.9 (Static Analysis)
     - CodeQL queries target known vulnerability classes (CWE-400, CWE-476, etc.).
       Results are reviewed during code review; SARIF is uploaded to GitHub Security tab.
       CodeQL is a widely-used, commercially-supported static analyser.

MSRV Pinning
------------

The Rust compiler version is pinned via a ``rust-toolchain.toml`` file at the repository
root. This file ensures every developer and CI runner uses exactly the same compiler,
preventing any inconsistency in code generation that could invalidate safety evidence.

.. code-block:: toml

   # rust-toolchain.toml — Minimum Supported Rust Version for Pullpiri (ISO 26262)
   [toolchain]
   channel = "1.92.0"
   components = ["rustfmt", "clippy", "rust-src"]

Qualification Audit Trail
--------------------------

+--------------------+------------------+-------------------+------------------------------------+
| Tool               | Evidence File    | CI Step           | Verified By                        |
+====================+==================+===================+====================================+
| rustc              | build logs       | Step 7 (build)    | Compiler output + cargo test        |
+--------------------+------------------+-------------------+------------------------------------+
| cargo test         | dist/tests/      | Step 10 (test)    | JUnit XML reports                   |
+--------------------+------------------+-------------------+------------------------------------+
| cargo-tarpaulin    | dist/coverage/   | Step 14 (coverage)| lcov.info + HTML + threshold gate   |
+--------------------+------------------+-------------------+------------------------------------+
| sphinx-build       | safety_docs/_build/| Step 14.6      | sphinx-build -W exit code           |
+--------------------+------------------+-------------------+------------------------------------+
| cargo clippy       | dist/reports/clippy/| Step 11 (lint)| clippy_summary.md                   |
+--------------------+------------------+-------------------+------------------------------------+
| cargo deny         | dist/reports/deny/| Step 13 (deny)  | deny_summary.md                     |
+--------------------+------------------+-------------------+------------------------------------+
| TSan               | dist/reports/    | tsan_sanitizer job| tsan_report.txt                    |
+--------------------+------------------+-------------------+------------------------------------+
| CodeQL             | dist/reports/    | codeql_scan job   | codeql_summary.md + SARIF          |
+--------------------+------------------+-------------------+------------------------------------+
