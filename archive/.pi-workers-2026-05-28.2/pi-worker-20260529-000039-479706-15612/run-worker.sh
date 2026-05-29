#!/usr/bin/env bash
set -u
cd /home/abhmul/dev/autocomplete-alternative
pi_bin=pi
tools=read\,grep\,find\,ls\,edit\,write\,bash
task=reports/orchestration/mvp-2026-05-28/tasks/MVP-060-docs-tdd-handoff.json
log=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-000039-479706-15612/output.log
status=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-000039-479706-15612/status
"$pi_bin" -p -t "$tools" "$task" </dev/null >"$log" 2>&1
code=$?
printf '%s\n' "$code" >"$status"
exit "$code"
