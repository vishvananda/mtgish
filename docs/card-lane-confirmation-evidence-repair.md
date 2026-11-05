# Repairing confirmation evidence

The verifier rejection limit stops automatic fix/verify cycles. It does not approve a card or erase prior findings.

When a coordinator repairs missing or overwritten evidence, `review_lane.py repair-confirmation JOB --folder REPAIR --validated-tree TREE --reason REASON` can request one fresh independent confirmation of the unchanged candidate. The repair directory supplies a passing review report, freshly rerun checks and their evidence, and the exact original `fixes.diff`. The tree must be clean and at that candidate, and the checks must match its complete work digest. Explain the discrepancy and its resolution in the repair report.

This action preserves the previous reports, rejection count and candidate. It creates a separate artifact round and records the coordinator's intervention. It grants neither approval nor another automatic fix cycle. Another rejection leaves the card blocked; the same candidate cannot repeatedly use this action. A code fix follows the ordinary review process instead.
