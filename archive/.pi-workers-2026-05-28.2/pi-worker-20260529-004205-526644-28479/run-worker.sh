#!/usr/bin/env bash
set -u
cd /home/abhmul/dev/autocomplete-alternative
pi_bin=pi
tools=read\,grep\,find\,ls\,edit\,write\,bash
task=reports/orchestration/mvp-2026-05-28/tasks/MVP-075-fix-obsidian-plugin-artifact.json
log=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-004205-526644-28479/output.log
status=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260529-004205-526644-28479/status
"$pi_bin" -p -t "$tools" "$task" </dev/null >"$log" 2>&1
code=$?
printf '%s\n' "$code" >"$status"
exit "$code"
