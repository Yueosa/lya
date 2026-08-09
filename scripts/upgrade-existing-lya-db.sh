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

# 若 persona.toml 存在，把 NULL/空人设会话补成当前默认正文
PERSONA_FILE="$(dirname "$DB")/persona.toml"
if [ -f "$PERSONA_FILE" ]; then
  PERSONA=$(python3 - <<'PY' "$PERSONA_FILE"
import sys, re
text = open(sys.argv[1], encoding="utf-8").read()
m = re.search(r'(?m)^text\s*=\s*"""([\s\S]*?)"""', text)
if m:
    print(m.group(1).strip())
else:
    m = re.search(r'(?m)^text\s*=\s*"([^"]*)"', text)
    print((m.group(1) if m else "").strip())
PY
)
  if [ -n "$PERSONA" ]; then
    sqlite3 "$DB" "UPDATE sessions SET persona = ?1 WHERE persona IS NULL OR persona = '';" "$PERSONA"
    echo "已用 persona.toml 补全空人设会话"
  fi
fi

echo "升级完成：$DB"
