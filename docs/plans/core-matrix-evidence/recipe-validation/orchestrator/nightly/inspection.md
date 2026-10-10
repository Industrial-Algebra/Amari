# Nightly phase-A flag inspection

Inspection verdict: PASS

- Source: `ba1b7719f609e8f0f127d620ba6936cfffb61413`.
- Matrix SHA-256: `40c43115dd04bc0569270ab9c1715e6912b311dc4aff1d49a5aee341db38c938`.
- Checkpoint SHA-256: `c59543bbf2199cc7ea4a411f289da22f86f8a8a4927e4eed18c23941fe85b510`.
- Lock SHA-256: `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`.
- Full `provenance.log`, `direct-cfg.txt` and `cargo-cfg.txt` inspected. Rustc/Cargo/Clippy/rustdoc are the researched nightly versions; direct/effective cfg select x86_64 Linux with fxsr/sse/sse2/x87 ISA feature entries. AVX/AVX2/FMA absent. Cargo cfg includes default/gf2/high-precision/phantom-types/std.
- Target compiler invocation: `cargo-cfg.log:3859`, plus library/test compilation in `check.log:7224,7803`; direct executable `/home/lucien/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/rustc`, target x86_64-unknown-linux-gnu, `-C target-cpu=x86-64 -C target-feature=-avx2`.
- Documenter invocation: `doc.log:3825`; direct `/home/lucien/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/rustdoc`, same target/baseline flags, `-D warnings`.
- The inspected target invocations contain no external sccache/invalid wrapper or inherited target-cpu=native/+avx2 flags, despite the checkpoint's deliberately contaminated inherited environment. Host build-script/proc-macro commands are distinct from these target commands.
- Phase A cfg/check/doc commands exited 0. No core library tests have executed yet in this checkpoint.

This is an orchestrator flag inspection, not an independent review, formal proof, or full-matrix certification.
