#!/usr/bin/env bash
# Traceability audit (tasks.md T072a; FR-018, SC-007; Constitution II).
#
# Every numeric constant in the core crate, and every literal tolerance in the harness checks,
# must cite a source or assumption (S-###, A-###), a requirement (FR-###, SC-###), or be labeled
# "engineering target", on the same line or within 2 lines above or below.
#
# Exempt: crates/rr_dr60/src/stages/voiceband_coeffs.rs (its generated header cites the IDs) and
# code inside #[cfg(test)] modules (test values are checked against the spec, not defaults).
set -euo pipefail
cd "$(dirname "$0")/.."

trace='A-[0-9]{3}|S-[0-9]{3}|FR-[0-9]{3}|SC-[0-9]{3}|engineering target'
status=0

check_file() {
  local file="$1" pattern="$2"
  awk -v pat="$pattern" -v trace="$trace" -v file="$file" '
    { line[NR] = $0 }
    /^#\[cfg\(test\)\]/ || /^    #\[cfg\(test\)\]/ { in_test = 1 }
    { if (!in_test && $0 ~ pat) hits[NR] = 1 }
    END {
      bad = 0
      for (n in hits) {
        ok = 0
        for (k = n - 2; k <= n + 2; k++) if ((k in line) && line[k] ~ trace) ok = 1
        if (!ok) { printf "%s:%d: no trace ID or \"engineering target\" near: %s\n", file, n, line[n]; bad = 1 }
      }
      exit bad
    }' "$file" || status=1
}

# Numeric constants in the core: `const NAME: <type> = <something with a digit>`.
while IFS= read -r f; do
  check_file "$f" '^[[:space:]]*(pub([(]crate[)])?[[:space:]]+)?const[[:space:]]+[A-Z0-9_]+:[^=]*=[^;]*[0-9]'
done < <(find crates/rr_dr60/src -name '*.rs' ! -name 'voiceband_coeffs.rs' | sort)

# Literal tolerances in the harness checks.
check_file crates/rr_dr60_harness/src/checks.rs 'Tolerance::(Range|AtLeast|AtMost|Exact)[(][-0-9]'
check_file crates/rr_dr60_harness/src/agc_checks.rs 'Tolerance::(Range|AtLeast|AtMost|Exact)[(][-0-9]' # spec 002

if [[ $status -eq 0 ]]; then
  echo "check-traceability: OK"
fi
exit $status
