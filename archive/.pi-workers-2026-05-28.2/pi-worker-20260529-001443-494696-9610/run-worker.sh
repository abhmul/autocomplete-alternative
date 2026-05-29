#!/usr/bin/env bash
set -u
cd /home/abhmul/dev/autocomplete-alternative
pi_bin=pi
tools=read\,grep\,find\,ls\,edit\,write\,bash
task=reports/orchestration/mvp-2026-05-28/tasks/MVP-071-real-editor-host-smoke.json
log=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-001443-494696-9610/output.log
status=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-001443-494696-9610/status
"$pi_bin" -p -t "$tools" "$task" </dev/null >"$log" 2>&1
code=$?
printf '%s\n' "$code" >"$status"
exit "$code"
