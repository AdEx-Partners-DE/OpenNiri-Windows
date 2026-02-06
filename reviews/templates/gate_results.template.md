# Gate Results - RV-YYYYMMDD-rNN

Date: YYYY-MM-DD

## Required Gates
- lint:
- typecheck:
- tests:
- build:

## Commands and Outcomes
- `command`: pass/fail
- `command`: pass/fail

## Scope Safety Checks
- Incident/hotfix mode active: yes/no
- One-variable-at-a-time honored: yes/no
- Dependency/framework upgrade included: yes/no
- If upgrade included, separate task/review cycle used: yes/no
- Lockfile impact summarized: yes/no

## Contract and Cross-Repo Checks
- Contract surface changed (payload/schema/API): yes/no
- Contract diff documented: yes/no
- Consumer impact expected: yes/no
- If consumer impact exists, change packet linked: yes/no

## Documentation Continuity Checks
- Current-state docs rewritten: yes/no
- If rewritten, historical context archived or referenced: yes/no

## Hygiene Checks
- Line-ending churn detected in unrelated files: yes/no
- Overlapping same-file lane ownership risk: yes/no

## Summary
- Blocking gates: none or list
- Decision: `ready_for_fix` or `ready_to_close`
