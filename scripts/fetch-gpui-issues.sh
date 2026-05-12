#!/usr/bin/env bash
# Regenerate docs/research/gpui-issues.md from the upstream zed-industries/zed
# repository. Merges manual triage from docs/research/gpui-issues-overlay.yaml.
# Requires: gh (authenticated), jq, python3 with PyYAML
# (the script imports `yaml` to convert the overlay YAML to JSON).
#
# Install PyYAML with `pip install pyyaml` if missing.
set -euo pipefail

# Preflight: confirm required tooling. PyYAML is only needed when an
# overlay exists; we still check it eagerly so the error fires before
# the long `gh api --paginate` fetch.
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

OUT="docs/research/gpui-issues.md"
OVERLAY_YAML="docs/research/gpui-issues-overlay.yaml"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

SRC="${TMP_DIR}/issues.jsonl"
OVERLAY_JSON="${TMP_DIR}/overlay.json"
OPEN_FILTER="${TMP_DIR}/open.jq"
CLOSED_FILTER="${TMP_DIR}/closed.jq"

TODAY="$(date +%Y-%m-%d)"

echo "Fetching issues with label area:gpui from zed-industries/zed..."
gh api --paginate "search/issues?q=repo:zed-industries/zed+label:area:gpui+is:issue&per_page=100" \
  --jq '.items[] | {n: .number, t: .title, s: .state, sr: .state_reason, ca: .created_at, cl: .closed_at, u: .user.login, l: [.labels[].name], url: .html_url, c: .comments}' \
  > "$SRC"

if [[ -f "$OVERLAY_YAML" ]]; then
  python -c "import yaml,json,sys; print(json.dumps(yaml.safe_load(open(sys.argv[1], encoding='utf-8')) or {}))" "$OVERLAY_YAML" > "$OVERLAY_JSON"
else
  echo '{}' > "$OVERLAY_JSON"
fi

OPEN_COUNT=$(jq -s 'map(select(.s=="open")) | length' "$SRC")
CLOSED_COUNT=$(jq -s 'map(select(.s=="closed")) | length' "$SRC")
TOTAL=$(jq -s 'length' "$SRC")

cat > "$OPEN_FILTER" <<'JQ'
def clean(x): (x // "") | tostring | gsub("\r"; "") | gsub("\n"; " ") | gsub("\\|"; "\\|") | gsub("\\["; "\\[") | gsub("\\]"; "\\]");
def lookup($n): ($overlay[0].issues // {})[$n | tostring] // {};
def adr_cell($n):  (lookup($n).adr  // "");
def repro_cell($n):(lookup($n).repro// "");
def notes_cell($n):(lookup($n).notes// "") | clean(.);
select(.s == "open")
| . as $i
| [
    "#" + (.n | tostring),
    clean(.t),
    (.ca // "")[0:10],
    "@" + (.u // "?"),
    ((.l // []) | map(select(. != "area:gpui")) | join(", ") | clean(.)),
    (.c // 0 | tostring),
    "[link](" + .url + ")",
    adr_cell(.n),
    repro_cell(.n),
    notes_cell(.n)
  ]
| "| " + join(" | ") + " |"
JQ

cat > "$CLOSED_FILTER" <<'JQ'
def clean(x): (x // "") | tostring | gsub("\r"; "") | gsub("\n"; " ") | gsub("\\|"; "\\|") | gsub("\\["; "\\[") | gsub("\\]"; "\\]");
def lookup($n): ($overlay[0].issues // {})[$n | tostring] // {};
def adr_cell($n):  (lookup($n).adr  // "");
def repro_cell($n):(lookup($n).repro// "");
def notes_cell($n):(lookup($n).notes// "") | clean(.);
select(.s == "closed")
| [
    "#" + (.n | tostring),
    clean(.t),
    (.cl // "")[0:10],
    (.sr // "completed"),
    "@" + (.u // "?"),
    ((.l // []) | map(select(. != "area:gpui")) | join(", ") | clean(.)),
    (.c // 0 | tostring),
    "[link](" + .url + ")",
    adr_cell(.n),
    repro_cell(.n),
    notes_cell(.n)
  ]
| "| " + join(" | ") + " |"
JQ

mkdir -p "$(dirname "$OUT")"

# Подсчёт ADR-кандидатов из overlay
ADR_YES=$(jq -r '(.issues // {}) | to_entries | map(select(.value.adr == "yes")) | length' "$OVERLAY_JSON")
ADR_MAYBE=$(jq -r '(.issues // {}) | to_entries | map(select(.value.adr == "maybe")) | length' "$OVERLAY_JSON")
OVERLAY_COUNT=$(jq -r '(.issues // {}) | length' "$OVERLAY_JSON")

{
  echo "# GPUI Issues — снимок"
  echo
  echo "**Источник:** [zed-industries/zed](https://github.com/zed-industries/zed/issues?q=label%3Aarea%3Agpui)"
  echo "**Фильтр:** \`label:area:gpui is:issue\`"
  echo "**Дата снимка:** ${TODAY}"
  echo "**Всего:** ${TOTAL}  (open: ${OPEN_COUNT}, closed: ${CLOSED_COUNT})"
  echo "**Overlay (наш триаж):** ${OVERLAY_COUNT} issues  (ADR: yes=${ADR_YES}, maybe=${ADR_MAYBE})"
  echo
  echo "Колонки \`ADR?\`, \`Reproduced in flui-v2?\`, \`Notes\` берутся из"
  echo "\`docs/research/gpui-issues-overlay.yaml\` — редактируйте overlay, а не этот файл (он перегенерируется)."
  echo
  echo "Расшифровка значений:"
  echo
  echo "- \`adr\`: \`yes\` — нужен ADR; \`maybe\` — пограничный; \`no\` — локальный bug; \`n-a\` — вне нашего scope."
  echo "- \`repro\`: \`yes\` / \`partial\` / \`no\` / \`unknown\` / \`n-a\` — повторили ли мы эту же проблему в flui-v2."
  echo
  echo "## Open (${OPEN_COUNT})"
  echo
  echo "| # | Title | Created | Author | Labels | Comments | Link | ADR? | Reproduced in flui-v2? | Notes |"
  echo "|---|-------|---------|--------|--------|----------|------|------|------------------------|-------|"
  jq -r --slurpfile overlay "$OVERLAY_JSON" -f "$OPEN_FILTER" "$SRC" | sort -t'#' -k2 -nr
  echo
  echo "## Closed (${CLOSED_COUNT})"
  echo
  echo "| # | Title | Closed | Reason | Author | Labels | Comments | Link | ADR? | Reproduced in flui-v2? | Notes |"
  echo "|---|-------|--------|--------|--------|--------|----------|------|------|------------------------|-------|"
  jq -r --slurpfile overlay "$OVERLAY_JSON" -f "$CLOSED_FILTER" "$SRC" | sort -t'#' -k2 -nr
  echo
  echo "---"
  echo
  echo "## Как пользоваться"
  echo
  echo "1. Все ручные пометки живут в \`docs/research/gpui-issues-overlay.yaml\` (ключ — номер issue)."
  echo "2. После правки overlay перегенерируйте этот файл: \`bash scripts/fetch-gpui-issues.sh\`."
  echo "3. Для \`adr: yes\` создаём ADR в \`docs/superpowers/specs/\` и/или \`docs/research/adr/\` со ссылкой на номер."
  echo "4. Поле \`repro\` отвечает на вопрос «повторили ли мы ту же проблему» — после проверки выставляйте \`yes\` / \`no\` / \`partial\`."
} > "$OUT"

echo "Wrote ${OUT} ($(wc -l < "$OUT") lines)"
echo "Overlay: ${OVERLAY_COUNT} entries (ADR yes=${ADR_YES}, maybe=${ADR_MAYBE})"
