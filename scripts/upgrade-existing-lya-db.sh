#!/usr/bin/env sh
# 升级已有 ~/.lya/lya.db（新装用户不需要）。
#
# 用法：./scripts/upgrade-existing-lya-db.sh
# 会先备份再执行 SQL。

set -eu

DB="${LYA_DB:-$HOME/.lya/lya.db}"
SQL="$(dirname "$0")/upgrade-existing-lya-db.sql"

if [ ! -f "$DB" ]; then
  echo "找不到 $DB" >&2
  exit 1
fi

BAK="${DB}.bak-$(date +%Y%m%d%H%M%S)"
cp "$DB" "$BAK"
echo "已备份到 $BAK"

sqlite3 "$DB" < "$SQL"

PROMPT_FILE="$(dirname "$DB")/prompt.toml"
if [ -f "$PROMPT_FILE" ]; then
  IDENTITY=$(python3 - <<'PY' "$PROMPT_FILE"
import sys, re, tomllib
with open(sys.argv[1], "rb") as f:
    data = tomllib.load(f)
print((data.get("identity") or {}).get("text") or "")
PY
)
  STYLE=$(python3 - <<'PY' "$PROMPT_FILE"
import sys, tomllib
with open(sys.argv[1], "rb") as f:
    data = tomllib.load(f)
print((data.get("style") or {}).get("text") or "")
PY
)
  if [ -n "$IDENTITY" ]; then
    sqlite3 "$DB" "UPDATE sessions SET identity = ?1 WHERE identity IS NULL OR identity = '';" "$IDENTITY"
    echo "已用 prompt.toml [identity] 补全空身份会话"
  fi
  if [ -n "$STYLE" ]; then
    sqlite3 "$DB" "UPDATE sessions SET style = ?1 WHERE style IS NULL OR style = '';" "$STYLE"
    echo "已用 prompt.toml [style] 补全空口吻会话"
  fi
fi

echo "升级完成：$DB"
