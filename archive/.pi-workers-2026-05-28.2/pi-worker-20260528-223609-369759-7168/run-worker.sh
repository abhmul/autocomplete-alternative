#!/usr/bin/env bash
set -u
cd /home/abhmul/dev/autocomplete-alternative
pi_bin=pi
tools=read\,grep\,find\,ls\,edit\,write\,bash
task=reports/orchestration/mvp-2026-05-28/tasks/MVP-010-protocol-core-tdd.json
log=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260528-223609-369759-7168/output.log
status=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260528-223609-369759-7168/status
"$pi_bin" -p -t "$tools" "$task" </dev/null >"$log" 2>&1
code=$?
printf '%s\n' "$code" >"$status"
exit "$code"
