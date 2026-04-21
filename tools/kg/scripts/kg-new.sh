#!/usr/bin/env bash
# Scaffold a new KG note under knowledge/<section>/<slug>.md.
# Usage:
#   tools/kg/scripts/kg-new.sh <section> "<Title>" [note-type] [tag1,tag2]
# Examples:
#   tools/kg/scripts/kg-new.sh concepts "Critical Path" concept wbs,scheduling
#   tools/kg/scripts/kg-new.sh decisions "Use xtask for builds" decision build
#
# Security:
#   - <section> must be one of the four known sections (no path
#     traversal through user input).
#   - <note-type> and each tag must not contain quotes, backslashes,
#     or newlines — these are interpolated into TOML frontmatter and
#     unescaped input would corrupt the generated file.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "${SCRIPT_DIR}/_lib.sh"

section="${1:-}"
title="${2:-}"
note_type="${3:-concept}"
tags="${4:-}"

if [[ -z "${section}" || -z "${title}" ]]; then
  sed -nE 's/^# //p; s/^#\s*//p' "${BASH_SOURCE[0]}" | head -n 10
  exit 2
fi

# Whitelist the section argument against the four known sections so a
# hostile or typo'd value cannot escape knowledge/ via path traversal.
case "${section}" in
  architecture|concepts|decisions|integrations) ;;
  *)
    echo "error: unknown section '${section}'" >&2
    echo "valid sections: architecture, concepts, decisions, integrations" >&2
    exit 2
    ;;
esac

# Reject dangerous characters in values that flow into generated TOML
# (note_type and each tag). We cannot trust shell quoting alone to
# protect the frontmatter layer — one embedded `"` and the TOML parse
# breaks; a newline and the frontmatter structure is free-form.
validate_toml_field() {
  local name="$1" value="$2"
  if [[ "${value}" == *$'\n'* || "${value}" == *$'\r'* \
        || "${value}" == *'"'* || "${value}" == *'\'* ]]; then
    echo "error: ${name} contains forbidden characters" >&2
    echo "       (reject: double-quote, backslash, newline, CR)" >&2
    exit 2
  fi
}

validate_toml_field "note-type" "${note_type}"

# Slugify the title: lowercase, non-alnum -> hyphen, squeeze, trim.
slug="$(printf '%s' "${title}" \
  | tr '[:upper:]' '[:lower:]' \
  | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"

if [[ -z "${slug}" ]]; then
  echo "error: title slugified to an empty string" >&2
  exit 2
fi

target="${KG_KNOWLEDGE}/${section}/${slug}.md"
if [[ -e "${target}" ]]; then
  echo "error: ${target} already exists" >&2
  exit 1
fi

mkdir -p "${KG_KNOWLEDGE}/${section}"

# Build [taxonomies] tags array from comma-separated list.
tags_toml=""
if [[ -n "${tags}" ]]; then
  IFS=',' read -ra arr <<< "${tags}"
  quoted=""
  for t in "${arr[@]}"; do
    t_trim="$(echo "${t}" | sed -E 's/^ +| +$//g')"
    [[ -z "${t_trim}" ]] && continue
    validate_toml_field "tag" "${t_trim}"
    quoted+="\"${t_trim}\", "
  done
  quoted="${quoted%, }"
  if [[ -n "${quoted}" ]]; then
    tags_toml="[taxonomies]
tags = [${quoted}]
"
  fi
fi

today="$(date -u +%Y-%m-%d)"
cat > "${target}" <<EOF
+++
title = "${title//\"/\\\"}"
date = ${today}
description = ""

${tags_toml}[extra]
note_type = "${note_type}"
# Add outgoing links as:
#   links = [
#     { relation = "relates-to", target = "concepts/task" },
#   ]
links = []
+++

<!-- Body starts here. Use markdown. -->
EOF

echo "created: ${target#"${REPO_ROOT}/"}"
