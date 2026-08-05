//! 聊天内媒体引用说明（不进 tool 段，每轮固定注入）。

/// 引用语法说明；不含「你能不能看图」的判断。
const SYNTAX: &str = "\
=== [界面] 聊天媒体 ===
用户界面能内联展示图片、视频、音频。要给用户看/听，**必须用 Markdown 语法引用**，不要只输出一行路径文字。

语法（与图片相同，靠扩展名区分类型）：
- 图片：`![描述](/home/用户/路径/photo.jpg)` 或 `![](https://…)`
- 视频：`![描述](/home/用户/路径/clip.mp4)` 或 `![](https://…/a.mp4)`
- 音频：`![描述](/home/用户/路径/song.mp3)` 或 `![](https://…/a.mp3)`

路径请用家目录绝对路径（`~/` 在部分工具里会展开）；远程只用 `http`/`https` URL。";

/// 模型看不见媒体内容时的断言。
const NO_VISION: &str = "\
你**看不到**这些图片、视频、音频里的内容，只能拿到路径、尺寸这类元信息。
用户问「这张图里是什么」时，如实说自己看不到，不要根据文件名猜测内容。
在界面里**展示**给用户看，是你能做的那部分。";

/// 模型能看图时的断言。
const VISION: &str = "\
本会话的模型能读懂**用户发给你的图片**内容，可以直接描述和分析。
视频与音频仍然看不到 / 听不到内容，你只能把它们展示给用户。
注意：你用 Markdown 引用的本地图片是给**用户界面**渲染的，不等于它被送进了你的输入。";

/// 组装聊天媒体段。
///
/// `vision` 来自 `models.toml` 的 capabilities，由调用方查好后传入——
/// 模型自己无从判断「本会话模型支不支持多模态」，把这种条件句写进提示词
/// 只会让它反复纠结，所以这里必须给一个确定的说法。
pub fn chat_media_section(vision: bool) -> String {
    let verdict = if vision { VISION } else { NO_VISION };
    format!("{SYNTAX}\n{verdict}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_is_definitive_either_way() {
        let off = chat_media_section(false);
        assert!(off.contains("看不到"));
        assert!(!off.contains("除非"), "不能再留「除非…」这种模型答不了的条件句");

        let on = chat_media_section(true);
        assert!(on.contains("能读懂"));
        assert!(!on.contains("除非"));
    }
}
