#!/usr/bin/env bash
# Validate that every target referenced in a note's [extra].links table
# resolves to an existing note under knowledge/.
# Exits non-zero on broken targets. Prints a summary at the end.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "${SCRIPT_DIR}/_lib.sh"

broken=0
total_links=0
notes=0

# For every .md file under knowledge/
while IFS= read -r -d '' note; do
  notes=$((notes + 1))
  # Extract target = "…" strings inside the frontmatter (between the
  # first +++ block).
  in_front=0
  front=""
  while IFS= read -r line; do
    if [[ "${line}" == "+++" ]]; then
      if (( in_front == 0 )); then in_front=1; continue; fi
      break
    fi
    (( in_front == 1 )) && front+="${line}"$'\n'
  done < "${note}"

  # Find every `target = "…"` in the frontmatter.
  while IFS= read -r target; do
    [[ -z "${target}" ]] && continue
    total_links=$((total_links + 1))
    # Strip leading slash, resolve against knowledge/.
    rel="${target#/}"
    candidate="${KG_KNOWLEDGE}/${rel}.md"
    if [[ ! -f "${candidate}" ]]; then
      # Section index?
      if [[ ! -f "${KG_KNOWLEDGE}/${rel}/_index.md" ]]; then
        echo "BROKEN  ${note##*/knowledge/}  ->  ${target}"
        broken=$((broken + 1))
      fi
    fi
  done < <(printf '%s' "${front}" | grep -oE 'target[[:space:]]*=[[:space:]]*"[^"]+"' | sed -E 's/.*"([^"]+)"/\1/')
done < <(find "${KG_KNOWLEDGE}" -type f -name '*.md' -print0)

echo ""
echo "notes:   ${notes}"
echo "links:   ${total_links}"
echo "broken:  ${broken}"
exit $(( broken > 0 ? 1 : 0 ))
