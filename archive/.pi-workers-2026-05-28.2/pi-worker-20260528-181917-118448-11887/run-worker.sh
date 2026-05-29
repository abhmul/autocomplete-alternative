#!/usr/bin/env bash
set -u
cd /home/abhmul/dev/autocomplete-alternative
pi_bin=pi
tools=read\,grep\,find\,ls\,edit\,write\,bash
task=reports/orchestration/tasks/T030-obsidian-options.json
log=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260528-181917-118448-11887/output.log
status=/home/abhmul/dev/autocomplete-alternative/.pi-workers/pi-worker-20260528-181917-118448-11887/status
"$pi_bin" -p -t "$tools" "$task" </dev/null >"$log" 2>&1
code=$?
printf '%s\n' "$code" >"$status"
exit "$code"
