<!--
  蔚蓝档案风格字标。

  # 构造是照着 logo 生成器的源码来的，不是凭印象

  那份 canvas 实现的绘制顺序与参数是：

  ```
  蓝色左半 #128AFA → 光环 → 近黑右半 #2B2B2B（12px 白描边）→ 十字
  horizontalTilt = -0.4      字号 84px      描边 12px
  ```

  `setTransform(1, 0, -0.4, 1, 0, 0)` 是一个错切：`x' = x - 0.4y`。CSS 的
  `skewX(θ)` 给的是 `x' = x + tan(θ)·y`，所以 `tan(θ) = -0.4`，θ ≈ -21.8°。
  描边 12/84 ≈ 0.14em，而 canvas 的 `strokeText` 是居中描边，向外只长一半，
  所以这里用 0.07em。

  # 为什么不用图片

  两个字 + 一个环 + 一条斜杠，纯 CSS 就够，省掉一份需要维护的资源。官方那支字体
  （RoG2 サンセリフ Std B）是 Morisawa 的商业字体，开源回退 Wêlai Glow Sans Heavy
  是个几 MB 的 CJK 字体——为一个字标下载它不值得，所以用系统重量级无衬线。

  # 颜色不在这里

  这里只写几何：错切角度、环的尺寸、斜杠的位置。上色在 `themes/ba.css`，和 `McShell`
  同一个规矩——主题专属组件也是组件，写死色值就等于给未来的变体（比如深色版）埋一处
  必须特判的地方。
-->

<script setup lang="ts">
withDefaults(
  defineProps<{
    /** 左半，蓝色。 */
    left?: string
    /** 右半，近黑带白描边。 */
    right?: string
  }>(),
  { left: 'lya', right: 'Archive' },
)
</script>

<template>
  <span class="ba-logo" aria-hidden="false">
    <span class="ba-logo__tilt">
      <span class="ba-logo__left">{{ left }}</span>
      <span class="ba-logo__halo" aria-hidden="true">
        <span class="ba-logo__slash" />
      </span>
      <span class="ba-logo__right">{{ right }}</span>
    </span>
  </span>
</template>

<style scoped>
.ba-logo {
  display: inline-block;
  /* 字号由外面给，内部一律用 em，整块能等比缩放 */
  font-family: var(--font-ui);
  font-weight: 900;
  letter-spacing: 0.01em;
  line-height: 1;
  white-space: nowrap;
  user-select: none;
}

.ba-logo__tilt {
  display: inline-flex;
  align-items: center;
  /* tan(-21.8°) = -0.4，与生成器的 horizontalTilt 一致 */
  transform: skewX(-21.8deg);
}

.ba-logo__right {
  /* 描边画在填充下面，否则会吃掉字形内部。描边色由主题给 */
  paint-order: stroke fill;
}

/*
 * 光环夹在两半中间，压着字。生成器里它的尺寸等于画布高度、往左偏 15/250 ≈ 0.06，
 * 这里换成相对字号的量。
 */
.ba-logo__halo {
  position: relative;
  display: inline-block;
  width: 0.62em;
  height: 0.62em;
  margin: 0 -0.06em;
  /* 环自己不跟着倾斜，否则会变成椭圆 */
  transform: skewX(21.8deg);
}

.ba-logo__halo::before {
  content: '';
  position: absolute;
  inset: 0;
  border-style: solid;
  border-width: 0.1em;
  border-radius: 50%;
}

/* 十字那条长斜杠：生成器用一个四点白色多边形在环上挖出缺口，这里用一条细矩形 */
.ba-logo__slash {
  position: absolute;
  top: -0.14em;
  left: 50%;
  width: 0.09em;
  height: 1.06em;
  transform: translateX(-50%) rotate(32deg);
}
</style>
