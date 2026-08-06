#!/usr/bin/env bash
#
# 给记忆大厅的视频瘦身。
#
# ## 先说实话：光去音轨省不下什么
#
# 量过手上这批：音轨占 1.9%–3.1%，一个 80 MB 的文件里只有 2 MB 是声音。去掉它文件
# 还是 78 MB，加载不会变快。
#
# 真正的问题是**码率**。这些是壁纸规格的素材：1080p 给到 6–10 Mbps，有一个 6 秒的
# 2560×1440 给到 43 Mbps。当背景放的话，2.5 Mbps 的 1080p 已经看不出差别，而文件能
# 小到四分之一（实测 61 MB 的那个编到 15 MB 左右）。
#
# 所以两档：
#
#   ./cg-slim.sh              去音轨。无损、秒完（只是重新封装，不重新编码）
#   ./cg-slim.sh --shrink     去音轨 + 重编码到 1080p / CRF 23。慢，但真的小
#   ./cg-slim.sh --dry-run    只看不动
#
# 目录默认 ~/.lya/theme/ba/cg，也可以当参数传，或者用 CG_DIR 环境变量。
#
# ## 三个踩过的坑
#
# 1. **ffmpeg 会读 stdin。** 它拿标准输入收交互命令，放进 `while read` 循环里就会
#    把剩下的文件列表整个吃掉，同时弹出 `Enter command:` 提示。所以要 `-nostdin`，
#    并且列表**先读进数组**再循环，不让两者共用 fd 0。
# 2. **临时文件会被自己的 find 捞回来。** `*.slim.tmp.mp4` 也匹配 `*.mp4`，上一次
#    中断留下的半成品会被当成输入，报 `partial file` / `Invalid NAL unit size`。
# 3. **中断要清理。** 不然下次一进来就撞上第 2 条。

set -uo pipefail

CG_DIR="${CG_DIR:-$HOME/.lya/theme/ba/cg}"
MODE="strip"
DRY=0
# 临时文件的后缀，三处要用同一个：生成、排除、清理
TMP_SUFFIX=".slim.tmp.mp4"

while [ $# -gt 0 ]; do
  case "$1" in
    --shrink) MODE="shrink" ;;
    --dry-run) DRY=1 ;;
    -h|--help) sed -n '2,32p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) CG_DIR="$1" ;;
  esac
  shift
done

command -v ffmpeg >/dev/null || { echo "要先装 ffmpeg" >&2; exit 1; }

# 小于 1 MB 的按 KB 说。整数除法会把小文件全显示成「0 MB」，看不出发生了什么
human() {
  if [ "$1" -ge 1048576 ]; then
    printf '%d MB' $(( $1 / 1048576 ))
  else
    printf '%d KB' $(( $1 / 1024 ))
  fi
}
[ -d "$CG_DIR" ] || { echo "找不到目录：$CG_DIR" >&2; exit 1; }

# 当前正在写的临时文件；Ctrl-C 时要把它删掉，别留给下一次当输入
current_tmp=""
cleanup() {
  [ -n "$current_tmp" ] && [ -f "$current_tmp" ] && rm -f "$current_tmp"
  echo
  echo "已中断。半成品清掉了，原件没动。"
  exit 130
}
trap cleanup INT TERM

# 先收拾上次留下的半成品：它们是截断的 mp4，留着会在下一轮被当成输入。
# 删不掉要出声——静默失败的话，下一步的排除规则一旦有缝就直接踩雷
while IFS= read -r -d '' stale; do
  if rm -f "$stale"; then
    echo "清掉上次中断留下的半成品：$(basename "$stale")"
  else
    echo "删不掉 $stale，请手动删掉再跑" >&2
    exit 1
  fi
done < <(find "$CG_DIR" -type f -name "*$TMP_SUFFIX" -print0)

# 重编码参数。CRF 23 是「肉眼基本无损」的常用档；背景视频还可以再往上调。
# -vf 只在超过 1080p 时缩，不放大；-movflags +faststart 把索引挪到文件头，
# 播放器不必先下完整个文件才能开始播——这对几十 MB 的背景很关键。
SHRINK_ARGS=(
  -c:v libx264 -preset slow -crf 23
  -vf "scale='min(1920,iw)':-2:flags=lanczos"
  -pix_fmt yuv420p -movflags +faststart -an
)

# 列表**先读进数组**：和 ffmpeg 共用 fd 0 的话，它会把剩下的条目吃掉
files=()
while IFS= read -r -d '' file; do
  files+=("$file")
done < <(find "$CG_DIR" -type f \( -iname '*.mp4' -o -iname '*.webm' \) \
  ! -name "*$TMP_SUFFIX" -print0 | sort -z)

[ ${#files[@]} -eq 0 ] && { echo "$CG_DIR 里没有视频"; exit 0; }

total_before=0
total_after=0
changed=0
skipped=0
failed=0
n=0

for file in "${files[@]}"; do
  n=$((n + 1))
  before=$(stat -c%s "$file")
  base=$(basename "$file")
  has_audio=$(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 "$file" 2>/dev/null || true)

  if [ "$MODE" = "strip" ] && [ -z "$has_audio" ]; then
    echo "[$n/${#files[@]}] 跳过（本来就没音轨）  $base"
    skipped=$((skipped + 1))
    total_before=$((total_before + before))
    total_after=$((total_after + before))
    continue
  fi

  if [ "$DRY" = "1" ]; then
    printf '[%d/%d] 会处理  %s  (%s)\n' "$n" "${#files[@]}" "$base" "$(human "$before")"
    continue
  fi

  printf '[%d/%d] %s  (%s) … ' "$n" "${#files[@]}" "$base" "$(human "$before")"

  tmp="${file%.*}$TMP_SUFFIX"
  current_tmp="$tmp"
  # -nostdin：不让 ffmpeg 抢标准输入，见文件头第 1 条
  if [ "$MODE" = "shrink" ]; then
    ffmpeg -nostdin -v error -y -i "$file" "${SHRINK_ARGS[@]}" "$tmp"
  else
    # -c copy：流拷贝，不重新编码，所以是无损且几乎不花时间
    ffmpeg -nostdin -v error -y -i "$file" -c copy -an "$tmp"
  fi
  status=$?
  current_tmp=""

  if [ $status -ne 0 ] || [ ! -s "$tmp" ]; then
    rm -f "$tmp"
    echo "失败（原件没动）"
    failed=$((failed + 1))
    total_before=$((total_before + before))
    total_after=$((total_after + before))
    continue
  fi

  after=$(stat -c%s "$tmp")
  # 只在真的变小了才替换：重编码偶尔会比原件大（原件本来就压得很狠时）
  if [ "$after" -lt "$before" ]; then
    mv -f "$tmp" "$file"
    changed=$((changed + 1))
    printf '%s  (-%d%%)\n' "$(human "$after")" $(( (before - after) * 100 / before ))
  else
    rm -f "$tmp"
    echo "跳过（处理后反而更大）"
    skipped=$((skipped + 1))
    after=$before
  fi

  total_before=$((total_before + before))
  total_after=$((total_after + after))
done

[ "$DRY" = "1" ] && exit 0

echo
echo "改了 $changed 个，跳过 $skipped 个，失败 $failed 个"
if [ "$total_before" -gt 0 ]; then
  printf '合计 %s → %s\n' "$(human "$total_before")" "$(human "$total_after")"
  if [ "$MODE" = "strip" ] && [ "$changed" -gt 0 ]; then
    echo
    echo "音轨本来就只占 2–3%。真想让它快，用 --shrink 重编码码率。"
  fi
fi
