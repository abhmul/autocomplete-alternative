#!/usr/bin/env bash
set -u
cd /home/abhmul/dev/autocomplete-alternative
pi_bin=pi
tools=read\,grep\,find\,ls\,edit\,write\,bash
task=reports/orchestration/mvp-2026-05-28/tasks/MVP-073-verify-strict-clippy-fix.json
log=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-003943-523550-22774/output.log
status=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-003943-523550-22774/status
"$pi_bin" -p -t "$tools" "$task" </dev/null >"$log" 2>&1
code=$?
printf '%s\n' "$code" >"$status"
exit "$code"
