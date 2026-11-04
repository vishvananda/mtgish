# Accented names in card preparation

Cantrip's bootstrap tool writes `dand_n.rs` for Dandân, while the lane's generated module is `dandan.rs`. Both files
survived preparation, so the scope gate rejected the extra bootstrap file. The tool also named its TODO scenarios
`dand-n-...`, outside the lane's canonical scenario ownership prefix.

Preparation now removes only an alternative module explicitly reported as freshly written by that invocation.
Removal requires a regular untracked file in the card directory containing exactly the tool's characteristics-only
stub. Existing tracked modules, symlinks, unrelated files, and files containing behavior are retained or rejected.
The destination is determined before invoking the tool, so a new legacy stub cannot change module selection.
Fresh TODO scenarios use the lane's canonical content slug, with collision checks before renaming.

Evidence under `~/card-lane/pipeline-unlimited-20261103/integration/`:

- `bootstrap-alias-pipeline-tests.log`: all 284 offline pipeline tests pass, including the new removal and ownership
  regressions.
- `bootstrap-alias-recovery/preparation-clean.json`: actual `prepare` in a clean native worktree leaves the correct
  Dandân module and correctly owned placeholders, with zero scope violations.
- `bootstrap-alias-recovery/runtime.json`: all six substantive scenarios from the stopped worker pass unchanged
  against the regenerated card, with native verification enabled. Its state-trigger citation is fixed by the earlier
  generator change. The two new TODO placeholders are still unfinished and must be replaced during ordinary work.

This is preparation validation, not card approval. Dandân retains its reservation and takes the regular pipeline
gates on retry. Oath of Lim-Dûl has the same naming problem plus an independent unreachable generated branch; the
naming change does not waive that remaining generator issue.
