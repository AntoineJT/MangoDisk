# Analysis scan metrics

`AnalysisScanMode::Standard` is the default for Core and adapters. It reports
native allocated bytes using the existing platform stream or traversal fallback.
On Windows, `Fast` fully traverses the same accessible scope and exclusions, but uses logical
file length from metadata without querying native allocation. Compressed and
sparse file lengths can differ substantially from their disk usage. Both modes
preserve no-follow traversal, hard-link charging, cancellation and delete preflight.

Results and in-memory directory aggregates carry the mode. Navigation, remainder
lists and delete reconciliation use that metric. Reusing an index in another mode
is forbidden; overlapping indexes in different metrics are invalidated rather than
patched. Deletions from another metric invalidate the incompatible index.

macOS and Linux expose one analysis action and always use `Standard`. Core normalizes
legacy fast requests to standard on these platforms and records the applied mode.
Windows defaults to standard on first use, restores the last selected mode on
startup, and discards navigation results when switching modes. Invalid saved
modes fall back to standard. Its labels describe standard bytes as disk usage and fast bytes
as file size. The standard CLI/Core convenience APIs retain their default behavior;
call `AnalysisService::analyze_with_mode_progress` to select a metric explicitly.

Analysis retains Windows-managed volume files in its space accounting. Entry
transport adds an optional `deletionProtection` hint (`systemManagedFile` or
`null`); adapters disable ordinary deletion for protected entries. Older cached
results may omit the hint, so Core always checks the platform classification
before opening any deletion target. This covers hibernation, paging, swap, and
DumpStack files at local volume roots, including verbatim and case variants,
local drive device paths, Win32 trailing-dot/space aliases, and file streams.
Verbatim trailing dots and spaces remain literal names rather than aliases.
Use the corresponding system setting instead of deleting these files directly.

## Interrupted scan diagnostics

Analysis logs identify stage transitions, rate-limited path/count progress, and Windows
worker activity (at most three active task samples per five seconds). Finalization and
result publication have separate events so a missing result can be localized beyond
directory enumeration. Cancellation is an informational outcome. Application startup
records include the process, executable and build profile; normal exit and Rust panic
events distinguish observable shutdown paths. Forced termination and native aborts may
leave no final event and must not be diagnosed as a specific crash from that absence alone.

On a disposable Windows machine, run native validation from a normal desktop token:

```powershell
$env:MANGODISK_ANALYSIS_VALIDATION_ROOT = 'C:\'
$env:MANGODISK_ANALYSIS_VALIDATION_DIR = Join-Path $PWD 'validation/analysis'
cargo test --manifest-path src-tauri/Cargo.toml -p mangodisk-core --test windows_analysis_validation -- --ignored --nocapture
```

The selected root is read-only. Unique fixtures and application storage stay under the
evidence directory. The test covers completion, cancellation, retry, ACL denial and an
invalid root, and verifies logging boundaries. Preserve local evidence outside commits.
