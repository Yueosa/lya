#!/usr/bin/env bash
#
# 构建 lya，产物放到 output/lya_<版本>_<系统>_<架构>/。
#
# 前端必须先构建：WebUI 是 rust-embed 从 web/dist/ 编进二进制的
# （见 crates/app/lya-api/src/http/static_ui.rs），漏掉这步会得到一个
# 能跑但打不开界面的 lya。
#
# 进度打到 stderr，最后一行 stdout 是产物路径，方便 install.sh 直接取。

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 有些环境（如编辑器沙箱）会用 CARGO_TARGET_DIR 把产物挪走，别写死 target/
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"

version="$(grep -m1 -E '^version[[:space:]]*=' "$repo_root/crates/app/lya/Cargo.toml" | cut -d'"' -f2)"
if [[ -z "$version" ]]; then
  echo "读不出版本号：crates/app/lya/Cargo.toml" >&2
  exit 1
fi

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"
out_dir="$repo_root/output/lya_${version}_${os}_${arch}"

echo "==> WebUI" >&2
npm --prefix "$repo_root/web" run build >&2

echo "==> lya $version (release)" >&2
cargo build --release --manifest-path "$repo_root/Cargo.toml" -p lya >&2

mkdir -p "$out_dir"
install -m 755 "$target_dir/release/lya" "$out_dir/lya"

echo "==> $out_dir/lya" >&2
echo "$out_dir/lya"
