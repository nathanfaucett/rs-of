# 04c: Join rejection and interrupted/exact retry

**Status:** resolved

**Blocked by:** 04b

Parent: 04. Bounded existing scenario-family migration; no production changes.

Generated setup and Device clients; reject pending membership, remain non-ready, exact retained retry, interruption acknowledgement is not rollback. Run both actual layouts.

Run targeted actual-process executions for both layouts before removing any wholly replaced old assertion body. Record concrete retained inventory in ticket04 report.

## Answer

Both actual layouts pass active exact retry, missing-capability cancellation denial, cancellation plus actual process interruption/reopen, exact protected intent and pending request retention, anonymous/authorized rejection, non-ready refusal, stale offer denial, and subsequent exact retry/approval (259.86s). Independent IdP/Storage waiting workers are explicitly cancelled/reopened after Device rejection. Stale offers: independent 409, Unified 404. Removed only the replaced public rejection/exact-request comparison block from api_join_tests; retained/renamed its private host-intent case, focused rerun passed (70.18s) after an initial unchanged reopen failure.
