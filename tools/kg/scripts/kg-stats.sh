#!/usr/bin/env bash
# Print KG stats: note count by section, total links, note types.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "${SCRIPT_DIR}/_lib.sh"

echo "RustWerk Knowledge Graph"
echo "========================"
echo "root:    ${KG_KNOWLEDGE#"${REPO_ROOT}/"}"
echo ""

total_notes=0
total_links=0

for section_dir in "${KG_KNOWLEDGE}"/*/; do
  [[ -d "${section_dir}" ]] || continue
  section_name="$(basename "${section_dir}")"
  # Count non-index notes
  count=$(find "${section_dir}" -maxdepth 1 -type f -name '*.md' ! -name '_index.md' | wc -l | tr -d ' ')
  printf "  %-14s %3s notes\n" "${section_name}" "${count}"
  total_notes=$((total_notes + count))
done

total_links=$(find "${KG_KNOWLEDGE}" -type f -name '*.md' -print0 \
  | xargs -0 grep -hoE 'target[[:space:]]*=[[:space:]]*"[^"]+"' \
  | wc -l | tr -d ' ')

echo ""
echo "total notes:  ${total_notes}"
echo "total links:  ${total_links}"
