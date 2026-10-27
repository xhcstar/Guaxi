<template>
  <!--
    阅读器左侧的标签侧栏。

    为什么抽成一个组件：ConvertPDF / Epub / Markdown / Text 四个阅读器原先各自在左边
    摆一列大纲，结构、拖拽把手、样式几乎逐行重复；现在统一成「顶部标签切换内容」，
    与 DocEbookReader 的目录、独立阅读器 ReaderPane 的侧栏观感一致。
    抽出来之后每个阅读器只需要提供各标签的内容，不再各自维护一套列布局。

    不自带任何业务数据：标签从 tabs 传入，内容用同名具名插槽提供，
    这样「大纲 / 书签 / 笔记」由各阅读器按自己有的数据决定，组件本身不需要知道它们。
  -->
  <aside
    class="rsp"
    :style="{ width: `${width}px` }"
    role="tablist"
    aria-label="阅读器侧栏"
  >
    <div class="rsp__head">
      <div class="rsp__tabs">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          class="rsp__tab"
          :class="{ 'is-on': active === tab.key }"
          role="tab"
          :aria-selected="active === tab.key"
          :title="tab.title || tab.label"
          @click="$emit('update:active', tab.key)"
        >
          {{ tab.label }}
          <span v-if="tab.count" class="rsp__tab-count">{{ tab.count }}</span>
        </button>
      </div>
      <button class="rsp__close" type="button" title="收起侧栏" aria-label="收起侧栏" @click="$emit('close')">
        ×
      </button>
    </div>

    <div class="rsp__body">
      <slot :name="active" :tab="active" />
    </div>

    <!-- 右侧拖拽把手：与原先大纲列的拖拽行为一致，宽度由宿主持有。 -->
    <div
      class="rsp__resize"
      role="separator"
      aria-orientation="vertical"
      aria-label="拖动调整侧栏宽度"
      @mousedown.prevent="$emit('resize-start', $event)"
    />
  </aside>
</template>

<script setup>
defineProps({
  /** [{ key, label, title?, count? }] */
  tabs: { type: Array, default: () => [] },
  /** 当前激活的标签 key。 */
  active: { type: String, default: '' },
  /** 侧栏宽度（px），由宿主持有以便持久化/拖拽。 */
  width: { type: Number, default: 260 },
})

defineEmits(['update:active', 'close', 'resize-start'])
</script>

<style scoped>
.rsp {
  position: relative;
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  min-width: 160px;
  max-width: 45%;
  height: 100%;
  border-right: 1px solid var(--guaxi-border, #e1e4e8);
  background: var(--guaxi-panel-bg, #fafbfc);
  color: var(--guaxi-text-primary, #1f2328);
}

.rsp__head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--guaxi-border, #e1e4e8);
}

.rsp__tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: 1 1 auto;
  min-width: 0;
}

.rsp__tab {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 9px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font-size: 12px;
  line-height: 1.4;
  cursor: pointer;
  white-space: nowrap;
  opacity: 0.72;
  transition: background 0.12s ease, opacity 0.12s ease;
}

.rsp__tab:hover {
  background: var(--guaxi-hover-bg, rgba(15, 23, 42, 0.06));
  opacity: 1;
}

.rsp__tab.is-on {
  background: var(--guaxi-selection-bg, rgba(0, 122, 204, 0.14));
  opacity: 1;
}

.rsp__tab-count {
  font-size: 10px;
  padding: 0 4px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.22);
  font-variant-numeric: tabular-nums;
}

.rsp__close {
  flex: 0 0 auto;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: inherit;
  font-size: 15px;
  line-height: 1;
  opacity: 0.6;
  cursor: pointer;
}

.rsp__close:hover {
  background: var(--guaxi-hover-bg, rgba(15, 23, 42, 0.06));
  opacity: 1;
}

.rsp__body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 6px 4px 12px;
}

/* 拖拽把手压在右边框上：1px 的边框不好瞄准，给一条 5px 的透明热区。 */
.rsp__resize {
  position: absolute;
  top: 0;
  right: -3px;
  bottom: 0;
  width: 5px;
  cursor: col-resize;
  z-index: 2;
}

.rsp__resize:hover {
  background: var(--guaxi-selection-bg, rgba(0, 122, 204, 0.18));
}
</style>