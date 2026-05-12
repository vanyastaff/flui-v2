#!/usr/bin/env bash
# Regenerate docs/research/flutter-issues.md from flutter/flutter.
# Snapshots open issues for the labels we consider relevant to flui-v2.
# Wide labels (c: performance, a: text input, ...) are capped to top-N
# by reactions to keep the artifact reviewable.
#
# Merges manual triage from docs/research/flutter-issues-overlay.yaml.
# Requires: gh (authenticated), jq, python3 with PyYAML, bash 4+
# (uses `declare -A`; on macOS the system /bin/bash is 3.2, install
# a newer bash via Homebrew or run with `bash` from PATH).
#
# Install PyYAML with `pip install pyyaml` if missing.
set -euo pipefail

# Preflight: bash 4+ for `declare -A`, required tooling, PyYAML.
if (( BASH_VERSINFO[0] < 4 )); then
  echo "ERROR: bash 4+ required (this script uses associative arrays). Found: ${BASH_VERSION}" >&2
  echo "  On macOS, install via 'brew install bash' and re-run with that bash on PATH." >&2
  exit 1
fi
for cmd in gh jq python; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "ERROR: required command '$cmd' not found on PATH" >&2
    exit 1
  fi
done
if ! python -c "import yaml" >/dev/null 2>&1; then
  echo "ERROR: Python module 'yaml' (PyYAML) not installed. Run: pip install pyyaml" >&2
  exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

OUT="docs/research/flutter-issues.md"
OVERLAY_YAML="docs/research/flutter-issues-overlay.yaml"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

OVERLAY_JSON="${TMP_DIR}/overlay.json"
ALL_JSON="${TMP_DIR}/all.jsonl"
TODAY="$(date +%Y-%m-%d)"

# label | mode | limit
# mode: all = fetch every open issue with the label
#       top = fetch top N by reactions (for wide labels)
declare -a LABELS=(
  "c: rendering|all|0"
  "c: API break|all|0"
  "c: new widget|all|0"
  "a: layout|all|0"
  "a: animation|all|0"
  "a: mouse|all|0"
  "a: gamedev|all|0"
  "c: performance|top|150"
  "a: typography|top|100"
  "a: accessibility|top|100"
)

# Helper: URL-encode a label
url_encode_label() {
  python -c "import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))" "$1"
}

> "$ALL_JSON"
declare -A SEEN

for entry in "${LABELS[@]}"; do
  IFS='|' read -r LABEL MODE LIMIT <<< "$entry"
  ENC_LABEL=$(url_encode_label "$LABEL")
  SUFFIX="+sort:reactions-desc"
  if [[ "$MODE" == "top" ]]; then
    MAX_PAGES=$(( (LIMIT + 99) / 100 ))
  else
    MAX_PAGES=10
  fi

  echo "Fetching label '${LABEL}' (${MODE}${LIMIT:+, top ${LIMIT}})..."
  PAGE=1
  FETCHED=0
  while [[ $PAGE -le $MAX_PAGES ]]; do
    if ! RESP=$(gh api "search/issues?q=repo:flutter/flutter+is:issue+is:open+label:\"${ENC_LABEL}\"${SUFFIX}&per_page=100&page=${PAGE}"); then
      echo "ERROR: gh api failed for label '${LABEL}' (page ${PAGE}); aborting to avoid an incomplete snapshot" >&2
      exit 1
    fi
    if [[ $PAGE -eq 1 && "$MODE" == "all" ]]; then
      # GitHub Search API caps pagination at ~1000 results regardless of
      # total_count. Warn when a label exceeds that — the snapshot
      # cannot claim completeness in that case.
      TOTAL_COUNT=$(echo "$RESP" | jq -r '.total_count // 0')
      if [[ $TOTAL_COUNT -gt 1000 ]]; then
        echo "WARN: label '${LABEL}' has ${TOTAL_COUNT} open issues; Search API caps at 1000 — snapshot will be truncated." >&2
      fi
    fi
    COUNT=$(echo "$RESP" | jq '.items | length')
    if [[ "$COUNT" == "0" ]]; then break; fi
    echo "$RESP" | jq -c --arg label "$LABEL" '
      .items[] | {
        n: .number, t: .title, s: .state,
        ca: .created_at, u: .user.login,
        l: [.labels[].name],
        url: .html_url, c: .comments,
        r: (.reactions.total_count // 0),
        primary_label: $label
      }
    ' >> "${TMP_DIR}/${LABEL//[^a-zA-Z0-9]/_}.jsonl"
    FETCHED=$((FETCHED + COUNT))
    if [[ "$MODE" == "top" ]] && [[ $FETCHED -ge $LIMIT ]]; then break; fi
    PAGE=$((PAGE + 1))
  done

  # Trim to limit for top mode
  FILE="${TMP_DIR}/${LABEL//[^a-zA-Z0-9]/_}.jsonl"
  if [[ -f "$FILE" ]]; then
    if [[ "$MODE" == "top" ]]; then
      head -n "$LIMIT" "$FILE" > "${FILE}.trim" && mv "${FILE}.trim" "$FILE"
    fi
    # Deduplicate against SEEN
    while IFS= read -r line; do
      n=$(echo "$line" | jq -r '.n')
      if [[ -z "${SEEN[$n]:-}" ]]; then
        SEEN[$n]=1
        echo "$line" >> "$ALL_JSON"
      fi
    done < "$FILE"
    KEPT=$(wc -l < "$FILE")
    echo "  ${KEPT} fetched"
  fi
done

TOTAL_UNIQ=$(wc -l < "$ALL_JSON" | tr -d ' ')
echo "Total unique issues across labels: ${TOTAL_UNIQ}"

# Overlay
if [[ -f "$OVERLAY_YAML" ]]; then
  python -c "import yaml,json,sys; print(json.dumps(yaml.safe_load(open(sys.argv[1], encoding='utf-8')) or {}))" "$OVERLAY_YAML" > "$OVERLAY_JSON"
else
  echo '{}' > "$OVERLAY_JSON"
fi

ROW_FILTER="${TMP_DIR}/row.jq"
cat > "$ROW_FILTER" <<'JQ'
def clean(x): (x // "") | tostring | gsub("\r"; "") | gsub("\n"; " ") | gsub("\\|"; "\\|") | gsub("\\["; "\\[") | gsub("\\]"; "\\]");
def lookup($n): ($overlay[0].issues // {})[$n | tostring] // {};
def adr_cell($n):   (lookup($n).adr   // "");
def repro_cell($n): (lookup($n).repro // "");
def notes_cell($n): (lookup($n).notes // "") | clean(.);
[
  "#" + (.n | tostring),
  clean(.t),
  (.ca // "")[0:10],
  (.r | tostring),
  (.c | tostring),
  "@" + (.u // "?"),
  ((.l // []) | map(select(test("^(c:|a:|f:|p:|framework$)"))) | join(", ") | clean(.)),
  "[link](" + .url + ")",
  adr_cell(.n),
  repro_cell(.n),
  notes_cell(.n)
]
| "| " + join(" | ") + " |"
JQ

# Counts per primary_label
LABEL_COUNTS=$(jq -s 'group_by(.primary_label) | map({label: .[0].primary_label, count: length})' "$ALL_JSON")

ADR_YES=$(jq -r '(.issues // {}) | to_entries | map(select(.value.adr == "yes")) | length' "$OVERLAY_JSON")
ADR_MAYBE=$(jq -r '(.issues // {}) | to_entries | map(select(.value.adr == "maybe")) | length' "$OVERLAY_JSON")
OVERLAY_COUNT=$(jq -r '(.issues // {}) | length' "$OVERLAY_JSON")

mkdir -p "$(dirname "$OUT")"

{
  echo "# Flutter Issues — снимок"
  echo
  echo "**Источник:** [flutter/flutter](https://github.com/flutter/flutter/issues)"
  echo "**Дата снимка:** ${TODAY}"
  echo "**Уникальных issues:** ${TOTAL_UNIQ}  (только \`open\`, дедуплицированы по номеру)"
  echo "**Overlay (наш триаж):** ${OVERLAY_COUNT} issues  (ADR: yes=${ADR_YES}, maybe=${ADR_MAYBE})"
  echo
  echo "Snapshot фильтрует только релевантные для flui-v2 категории (рендеринг, layout,"
  echo "animation, mouse/gestures, typography, accessibility, performance). Широкие labels"
  echo "(\`c: performance\`, \`a: typography\`, \`a: accessibility\`) ограничены top-N по reactions."
  echo "Полный список фильтров см. в \`scripts/fetch-flutter-issues.sh\`."
  echo
  echo "Колонки \`ADR?\`, \`Reproduced in flui-v2?\`, \`Notes\` берутся из"
  echo "\`docs/research/flutter-issues-overlay.yaml\` — редактируйте overlay, не этот файл."
  echo
  echo "## Объём по категориям"
  echo
  echo "| Категория | Issues в снимке |"
  echo "|-----------|-----------------|"
  echo "$LABEL_COUNTS" | jq -r '.[] | "| \(.label) | \(.count) |"'
  echo
  echo "## Issues"
  echo

  # Group sections by primary_label
  for entry in "${LABELS[@]}"; do
    IFS='|' read -r LABEL MODE LIMIT <<< "$entry"
    LABEL_COUNT=$(jq -s "[.[] | select(.primary_label == \"${LABEL}\")] | length" "$ALL_JSON")
    if [[ "$LABEL_COUNT" == "0" ]]; then continue; fi
    echo "### ${LABEL} (${LABEL_COUNT})"
    echo
    echo "| # | Title | Created | 👍 | 💬 | Author | Labels | Link | ADR? | Reproduced in flui-v2? | Notes |"
    echo "|---|-------|---------|---|---|--------|--------|------|------|------------------------|-------|"
    jq -c --arg lab "$LABEL" 'select(.primary_label == $lab)' "$ALL_JSON" \
      | jq -r --slurpfile overlay "$OVERLAY_JSON" -f "$ROW_FILTER"
    echo
  done

  echo "---"
  echo
  echo "## Как пользоваться"
  echo
  echo "1. Все ручные пометки в \`docs/research/flutter-issues-overlay.yaml\` (ключ — номер issue)."
  echo "2. После правки overlay: \`bash scripts/fetch-flutter-issues.sh\`."
  echo "3. Для \`adr: yes\` создаём ADR в \`docs/superpowers/specs/\` или \`docs/research/adr/\`."
  echo "4. Если хочется добавить категорию — расширь массив \`LABELS\` в скрипте."
} > "$OUT"

echo "Wrote ${OUT} ($(wc -l < "$OUT") lines)"
