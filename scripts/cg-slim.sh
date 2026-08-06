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
# 小到四分之一。
#
# 所以两档：
#
#   ./cg-slim.sh              去音轨。无损、秒完（只是重新封装，不重新编码）
#   ./cg-slim.sh --shrink     去音轨 + 重编码到 1080p / CRF 23。慢，但真的小
#
# 两档都**先写临时文件、成功了才替换**，中途断电不会毁掉原件。加 --dry-run 只看不动。
#
# 目录默认 ~/.lya/theme/ba/cg，也可以当参数传。

set -euo pipefail

CG_DIR="${CG_DIR:-$HOME/.lya/theme/ba/cg}"
MODE="strip"
DRY=0

while [ $# -gt 0 ]; do
  case "$1" in
    --shrink) MODE="shrink" ;;
    --dry-run) DRY=1 ;;
    -h|--help) sed -n '2,30p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) CG_DIR="$1" ;;
  esac
  shift
done

command -v ffmpeg >/dev/null || { echo "要先装 ffmpeg" >&2; exit 1; }
[ -d "$CG_DIR" ] || { echo "找不到目录：$CG_DIR" >&2; exit 1; }

# 重编码参数。CRF 23 是「肉眼基本无损」的常用档；背景视频还可以再往上调。
# -vf 只在超过 1080p 时缩，不放大；-movflags +faststart 把索引挪到文件头，
# 播放器不必先下完整个文件才能开始播——这对几十 MB 的背景很关键。
SHRINK_ARGS=(
  -c:v libx264 -preset slow -crf 23
  -vf "scale='min(1920,iw)':-2:flags=lanczos"
  -pix_fmt yuv420p -movflags +faststart -an
)

total_before=0
total_after=0
changed=0
skipped=0

# -print0 / read -d ''：文件名里有空格、括号和中文，按行读会散架
while IFS= read -r -d '' file; do
  before=$(stat -c%s "$file")
  has_audio=$(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 "$file" || true)

  if [ "$MODE" = "strip" ] && [ -z "$has_audio" ]; then
    echo "跳过（本来就没音轨）  $(basename "$file")"
    skipped=$((skipped + 1))
    total_before=$((total_before + before))
    total_after=$((total_after + before))
    continue
  fi

  if [ "$DRY" = "1" ]; then
    echo "会处理  $(basename "$file")  ($((before / 1048576)) MB)"
    continue
  fi

  tmp="${file%.*}.slim.tmp.mp4"
  if [ "$MODE" = "shrink" ]; then
    ffmpeg -v error -y -i "$file" "${SHRINK_ARGS[@]}" "$tmp"
  else
    # -c copy：流拷贝，不重新编码，所以是无损且几乎不花时间
    ffmpeg -v error -y -i "$file" -c copy -an "$tmp"
  fi

  after=$(stat -c%s "$tmp")
  # 只在真的变小了才替换：重编码偶尔会比原件大（原件本来就压得很狠时）
  if [ "$after" -lt "$before" ]; then
    mv -f "$tmp" "$file"
    changed=$((changed + 1))
    printf '%-46s %5d MB → %5d MB  (-%d%%)\n' \
      "$(basename "$file")" $((before / 1048576)) $((after / 1048576)) \
      $(( (before - after) * 100 / before ))
  else
    rm -f "$tmp"
    echo "跳过（处理后反而更大）  $(basename "$file")"
    skipped=$((skipped + 1))
    after=$before
  fi

  total_before=$((total_before + before))
  total_after=$((total_after + after))
done < <(find "$CG_DIR" -type f \( -iname '*.mp4' -o -iname '*.webm' \) -print0)

[ "$DRY" = "1" ] && exit 0

echo
echo "改了 $changed 个，跳过 $skipped 个"
if [ "$total_before" -gt 0 ]; then
  printf '合计 %d MB → %d MB\n' $((total_before / 1048576)) $((total_after / 1048576))
  if [ "$MODE" = "strip" ] && [ "$changed" -gt 0 ]; then
    echo
    echo "音轨本来就只占 2–3%。真想让它快，用 --shrink 重编码码率。"
  fi
fi
