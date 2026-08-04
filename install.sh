#!/usr/bin/env bash
#
# 构建并安装到 ~/.local/bin/lya。

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest_dir="${HOME}/.local/bin"

binary="$("$repo_root/build.sh")"

mkdir -p "$dest_dir"

# 先落临时文件再 rename：同分区的 mv 是原子的，旧 inode 还在跑也换得掉。
# 直接往 ~/.local/bin/lya 上写会在 lya 运行时报 "Text file busy"。
tmp="$(mktemp "$dest_dir/.lya.XXXXXX")"
trap 'rm -f "$tmp"' EXIT
cp "$binary" "$tmp"
chmod 755 "$tmp"
mv -f "$tmp" "$dest_dir/lya"
trap - EXIT

echo "==> 已安装 $dest_dir/lya" >&2

case ":${PATH}:" in
  *":${dest_dir}:"*) ;;
  *) echo "提醒：$dest_dir 不在 PATH 里" >&2 ;;
esac
