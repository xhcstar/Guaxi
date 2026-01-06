<template>
  <n-message-provider placement="bottom">
    <div class="page">
      <div class="header">
        <div class="title">模板管理器</div>
        <n-space>
          <n-button size="small" @click="refresh" :loading="loading">刷新</n-button>
          <n-button size="small" @click="importFromFile" :disabled="loading">导入 Markdown 文件</n-button>
          <n-button size="small" @click="newTemplate" :disabled="loading">新建</n-button>
        </n-space>
      </div>

      <div class="path" :title="templateRepoPath">
        {{ templateRepoPath || '未配置模板库路径' }}
      </div>

      <div class="main">
        <div class="left">
          <n-input v-model:value="filter" size="small" placeholder="搜索模板..." clearable />
          <n-collapse accordion>
            <n-collapse-item
              v-for="group in groupedTemplates"
              :key="group.type"
              :title="`${group.type} (${group.items.length})`"
              :name="group.type"
            >
              <n-list hoverable clickable>
                <n-list-item
                  v-for="t in group.items"
                  :key="t.name"
                  :class="{ active: t.name === selectedName }"
                  @click="selectTemplate(t.name)"
                >
                  <div class="row">
                    <div class="row-name">
                      <span class="row-name-text">{{ t.name }}</span>
                      <span v-if="t.builtin" class="builtin-tag">内置</span>
                    </div>
                    <n-button
                      size="tiny"
                      tertiary
                      @click.stop="insertToEditor(t.name)"
                      :disabled="loading"
                    >
                      插入
                    </n-button>
                  </div>
                </n-list-item>
              </n-list>
            </n-collapse-item>
          </n-collapse>
        </div>

        <div class="right">
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="名称">
              <n-input v-model:value="editName" :disabled="loading" placeholder="例如：会议纪要" />
            </n-form-item>
            <n-form-item label="类型">
              <n-input v-model:value="editType" :disabled="loading" placeholder="例如：会议 / 日记 / TODO" />
            </n-form-item>
            <n-form-item label="内容">
              <n-input
                v-model:value="editContent"
                type="textarea"
                :disabled="loading"
                placeholder="输入模板内容（Markdown）"
                :autosize="{ minRows: 16, maxRows: 28 }"
              />
            </n-form-item>
          </n-form>

          <n-space justify="end">
            <n-button type="primary" :disabled="loading || !editName" @click="save">保存</n-button>
            <n-button
              tertiary
              type="error"
              :disabled="loading || !selectedName"
              @click="remove"
            >
              删除
            </n-button>
          </n-space>
        </div>
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { NButton, NCollapse, NCollapseItem, NForm, NFormItem, NInput, NList, NListItem, NSpace, createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const { message } = createDiscreteApi(['message'])

const loading = ref(false)
const templateRepoPath = ref('')
const templates = ref([])
const filter = ref('')

const selectedName = ref('')
const editName = ref('')
const editType = ref('未分类')
const editContent = ref('')

const DEFAULT_TEMPLATES = [
  {
    name: '会议纪要（通用）',
    templateType: '会议',
    content: '# 会议纪要\n\n## 基本信息\n\n| 项目 | 内容 |\n| --- | --- |\n| 会议主题 |  |\n| 时间 |  |\n| 地点/会议链接 |  |\n| 主持人 |  |\n| 记录人 |  |\n| 参会人员 |  |\n| 缺席人员 |  |\n\n---\n\n## 会议目标\n\n- 目标 1：\n- 目标 2：\n\n## 议程\n\n1. \n2. \n3. \n\n---\n\n## 讨论纪要（按议题）\n\n### 议题 1：\n\n- 背景：\n- 讨论要点：\n  - \n- 结论/共识：\n- 未决问题：\n  - \n\n### 议题 2：\n\n- 背景：\n- 讨论要点：\n  - \n- 结论/共识：\n- 未决问题：\n  - \n\n---\n\n## 决议（Decisions）\n\n| 序号 | 决议 | 影响范围 | 生效时间 | 备注 |\n| --- | --- | --- | --- | --- |\n| 1 |  |  |  |  |\n\n## 行动项（Action Items）\n\n| 序号 | 事项 | 负责人 | 截止日期 | 状态 | 关联议题 |\n| --- | --- | --- | --- | --- | --- |\n| 1 |  |  |  | 待开始 |  |\n\n## 风险与依赖\n\n- 风险：\n- 依赖：\n\n## 附录\n\n- 资料链接：\n- 截图/录屏：\n'
  },
  {
    name: '日报',
    templateType: '工作',
    content: '# 日报\n\n## 基本信息\n\n- 日期：\n- 负责人：\n- 所属项目/团队：\n\n---\n\n## 今日完成（Done）\n\n- [ ] 事项 1（结果/产出：）\n- [ ] 事项 2（结果/产出：）\n\n## 今日产出物\n\n| 类型 | 链接/路径 | 说明 |\n| --- | --- | --- |\n| PR/提交 |  |  |\n| 文档 |  |  |\n| 会议纪要 |  |  |\n\n## 进展与数据（可选）\n\n- 进度：%\n- 关键指标：\n\n---\n\n## 问题/风险（Blockers & Risks）\n\n| 问题/风险 | 影响 | 当前状态 | 需要谁协助 |\n| --- | --- | --- | --- |\n|  |  |  |  |\n\n## 明日计划（Next）\n\n- [ ] 事项 1\n- [ ] 事项 2\n\n## 备注\n\n- \n'
  },
  {
    name: '周报',
    templateType: '工作',
    content: '# 周报\n\n## 基本信息\n\n- 周期：\n- 负责人：\n- 项目/团队：\n\n---\n\n## 本周概览\n\n- 本周重点：\n- 总体进度：\n\n## 本周产出\n\n| 类别 | 条目 | 说明 |\n| --- | --- | --- |\n| 功能 |  |  |\n| 修复 |  |  |\n| 性能/体验 |  |  |\n| 文档/流程 |  |  |\n\n## 关键数据（可选）\n\n| 指标 | 本周 | 上周 | 环比 | 备注 |\n| --- | --- | --- | --- | --- |\n|  |  |  |  |  |\n\n---\n\n## 问题与风险\n\n- 问题：\n- 风险：\n- 复盘点：\n\n## 下周计划\n\n- [ ] 事项 1（交付物：）\n- [ ] 事项 2（交付物：）\n\n## 需要协助\n\n| 事项 | 需要谁 | 希望时间 | 说明 |\n| --- | --- | --- | --- |\n|  |  |  |  |\n'
  },
  {
    name: 'TODO（清单）',
    templateType: 'TODO',
    content: '# TODO\n\n## 今日（Today）\n\n- [ ] 最重要的一件事：\n- [ ] \n- [ ] \n\n## 本周（This Week）\n\n- [ ] \n- [ ] \n\n## 待办池（Backlog）\n\n- [ ] \n- [ ] \n\n---\n\n## 进行中（WIP）\n\n| 事项 | 优先级 | 预计完成 | 状态 | 备注 |\n| --- | --- | --- | --- | --- |\n|  | P1 |  | 进行中 |  |\n\n## 已完成（Done）\n\n- [x] \n'
  },
  {
    name: '阅读笔记',
    templateType: '学习',
    content: '# 阅读笔记\n\n## 书目信息\n\n| 项目 | 内容 |\n| --- | --- |\n| 书/文章 |  |\n| 作者 |  |\n| 来源/链接 |  |\n| 阅读时间 |  |\n| 主题标签 |  |\n\n---\n\n## 一句话总结\n\n> \n\n## 摘要（Summary）\n\n- \n\n## 核心观点（Key Points）\n\n1. \n2. \n3. \n\n## 证据与例子（Evidence）\n\n- 例子 1：\n- 例子 2：\n\n## 金句/引用（Quotes）\n\n> \n\n## 我的思考（Thoughts）\n\n- 同意的点：\n- 不同意的点：\n- 可以迁移到哪里：\n\n## 可行动项（Actions）\n\n- [ ] \n- [ ] \n\n## 相关链接\n\n- \n'
  },
  {
    name: '学习笔记（课程/视频）',
    templateType: '学习',
    content: '# 学习笔记\n\n## 基本信息\n\n- 主题：\n- 来源：\n- 学习时间：\n- 讲师/作者：\n- 关联项目：\n\n---\n\n## 学习目标\n\n- 目标 1：\n- 目标 2：\n\n## 知识点结构\n\n- 概念：\n- 方法：\n- 工具：\n- 注意事项：\n\n## 关键结论\n\n1. \n2. \n\n## 例子 / 代码\n\n```\n\n```\n\n## 常见坑\n\n- \n\n## 疑问与待查\n\n- [ ] \n\n## 复盘\n\n- 今天最大的收获：\n- 下一步练习：\n'
  },
  {
    name: '项目需求（PRD 简版）',
    templateType: '项目',
    content: '# 需求说明（PRD 简版）\n\n## 基本信息\n\n| 项目 | 内容 |\n| --- | --- |\n| 需求名称 |  |\n| 负责人 |  |\n| 关联项目 |  |\n| 版本/里程碑 |  |\n| 优先级 | P0 / P1 / P2 |\n| 评审时间 |  |\n\n---\n\n## 背景与问题\n\n- 现状：\n- 痛点：\n- 为什么现在做：\n\n## 目标（Goals）\n\n- 业务目标：\n- 用户目标：\n- 成功指标（可量化）：\n\n## 非目标（Non-goals）\n\n- 本期不做：\n\n---\n\n## 用户画像与场景\n\n- 用户 A：\n- 场景：\n\n## 方案概述\n\n- 方案一：\n- 方案二：\n- 选型结论：\n\n## 需求范围\n\n### In Scope\n\n- \n\n### Out of Scope\n\n- \n\n---\n\n## 功能列表\n\n| 模块 | 功能点 | 描述 | 优先级 | 备注 |\n| --- | --- | --- | --- | --- |\n|  |  |  | P1 |  |\n\n## 交互/原型\n\n- 原型链接：\n- 关键页面说明：\n\n## 数据与埋点（可选）\n\n| 事件 | 触发时机 | 属性 | 用途 |\n| --- | --- | --- | --- |\n|  |  |  |  |\n\n## 风险与依赖\n\n- 风险：\n- 依赖：\n\n## 验收标准\n\n- [ ] 功能验收：\n- [ ] 性能/稳定性：\n- [ ] 兼容性：\n- [ ] 回滚预案：\n'
  },
  {
    name: 'Bug 报告',
    templateType: '项目',
    content: '# Bug 报告\n\n## 基本信息\n\n- 标题：\n- 严重程度：致命 / 高 / 中 / 低\n- 优先级：P0 / P1 / P2\n- 影响范围：\n- 发现版本/分支：\n- 环境：系统/浏览器/设备/网络\n\n---\n\n## 复现步骤\n\n1. \n2. \n3. \n\n## 实际结果（Actual）\n\n- \n\n## 期望结果（Expected）\n\n- \n\n---\n\n## 复现概率\n\n- 必现 / 高概率 / 低概率 / 偶现\n\n## 影响评估\n\n- 用户影响：\n- 业务影响：\n- 是否可绕过：是 / 否（绕过方式：）\n\n## 日志/截图/录屏\n\n- \n\n## 初步定位（可选）\n\n- 可疑模块：\n- 最近变更：\n- 相关提交/PR：\n\n## 修复建议（可选）\n\n- \n\n## 验证要点\n\n- [ ] 主路径\n- [ ] 边界条件\n- [ ] 回归范围\n'
  },
  {
    name: '日记',
    templateType: '日记',
    content: '# 日记\n\n## 今日信息\n\n- 日期：\n- 心情：\n- 天气：\n- 关键词：\n\n---\n\n## 记录\n\n### 今天发生了什么\n\n- \n\n### 今天我做得好的地方\n\n- \n\n### 今天的困扰与应对\n\n- 困扰：\n- 我采取的行动：\n- 结果：\n\n---\n\n## 反思与成长\n\n- 我学到了什么：\n- 下一次我想怎么做：\n\n## 感谢\n\n- \n\n## 明日计划\n\n- [ ] \n'
  }
]

const hasUserTemplate = (name) => {
  const n = String(name || '').trim()
  if (!n) return false
  return (templates.value || []).some(t => String(t?.name || '').trim() === n)
}

const getBuiltInTemplate = (name) => {
  const n = String(name || '').trim()
  if (!n) return null
  return DEFAULT_TEMPLATES.find(t => String(t?.name || '').trim() === n) || null
}

const refresh = async () => {
  try {
    loading.value = true
    const [path, list] = await Promise.all([
      invoke('get_template_repo_path'),
      invoke('list_templates')
    ])
    templateRepoPath.value = typeof path === 'string' ? path : ''
    templates.value = Array.isArray(list) ? list : []
  } catch (e) {
    console.error('加载模板失败:', e)
    message.error('加载模板失败')
  } finally {
    loading.value = false
  }
}

const mergedTemplates = computed(() => {
  const byName = new Map()

  for (const t of DEFAULT_TEMPLATES) {
    const name = String(t?.name || '').trim()
    if (!name) continue
    const tp = String(t?.templateType || '未分类').trim() || '未分类'
    byName.set(name, {
      name,
      template_type: tp,
      builtin: true,
    })
  }

  for (const t of templates.value || []) {
    const name = String(t?.name || '').trim()
    if (!name) continue
    const tp = String(t?.template_type || '未分类').trim() || '未分类'
    byName.set(name, {
      name,
      template_type: tp,
      builtin: false,
    })
  }

  const q = filter.value.trim().toLowerCase()
  let list = Array.from(byName.values())
  if (q) {
    list = list.filter(t => {
      const n = String(t?.name || '').toLowerCase()
      const tp = String(t?.template_type || '').toLowerCase()
      return n.includes(q) || tp.includes(q)
    })
  }

  return list
})

const groupedTemplates = computed(() => {
  const map = new Map()
  for (const t of mergedTemplates.value) {
    const tp = String(t?.template_type || '未分类') || '未分类'
    if (!map.has(tp)) map.set(tp, [])
    map.get(tp).push(t)
  }
  return Array.from(map.entries()).map(([type, items]) => ({
    type,
    items: items.slice().sort((a, b) => String(a.name).localeCompare(String(b.name)))
  }))
})

const newTemplate = () => {
  selectedName.value = ''
  editName.value = ''
  editType.value = '未分类'
  editContent.value = ''
}

const selectTemplate = async (name) => {
  const n = String(name || '').trim()
  if (!n) return

  try {
    loading.value = true

    if (hasUserTemplate(n)) {
      const data = await invoke('read_template', { name: n })
      selectedName.value = data?.name || n
      editName.value = data?.name || n
      editType.value = data?.template_type || '未分类'
      editContent.value = data?.content || ''
      return
    }

    const builtIn = getBuiltInTemplate(n)
    if (builtIn) {
      selectedName.value = builtIn.name
      editName.value = builtIn.name
      editType.value = builtIn.templateType || '未分类'
      editContent.value = builtIn.content || ''
      return
    }

    const data = await invoke('read_template', { name: n })
    selectedName.value = data?.name || n
    editName.value = data?.name || n
    editType.value = data?.template_type || '未分类'
    editContent.value = data?.content || ''
  } catch (e) {
    console.error('读取模板失败:', e)
    message.error('读取模板失败')
  } finally {
    loading.value = false
  }
}

const save = async () => {
  const name = editName.value.trim()
  if (!name) {
    message.warning('请输入模板名称')
    return
  }

  try {
    loading.value = true

    if (selectedName.value && selectedName.value !== name && hasUserTemplate(selectedName.value)) {
      await invoke('rename_template', { oldName: selectedName.value, newName: name })
      selectedName.value = name
    } else {
      selectedName.value = name
    }

    await invoke('save_template', {
      name,
      templateType: editType.value.trim() || '未分类',
      content: editContent.value || ''
    })

    message.success('已保存')
    await refresh()
  } catch (e) {
    console.error('保存模板失败:', e)
    message.error('保存模板失败')
  } finally {
    loading.value = false
  }
}

const remove = async () => {
  if (!selectedName.value) return

  if (!hasUserTemplate(selectedName.value) && getBuiltInTemplate(selectedName.value)) {
    message.info('内置模板不可删除')
    return
  }

  const ok = window.confirm(`确认删除模板「${selectedName.value}」？`)
  if (!ok) return

  try {
    loading.value = true
    await invoke('delete_template', { name: selectedName.value })
    message.success('已删除')
    newTemplate()
    await refresh()
  } catch (e) {
    console.error('删除模板失败:', e)
    message.error('删除模板失败')
  } finally {
    loading.value = false
  }
}

const insertToEditor = async (name) => {
  const n = String(name || '').trim()
  if (!n) return

  try {
    loading.value = true

    if (hasUserTemplate(n)) {
      const data = await invoke('read_template', { name: n })
      const content = typeof data?.content === 'string' ? data.content : ''
      if (!content) return
      await invoke('emit_template_insert', { content })
      message.success('已发送到编辑器窗口')
      return
    }

    const builtIn = getBuiltInTemplate(n)
    const content = typeof builtIn?.content === 'string' ? builtIn.content : ''
    if (!content) return
    await invoke('emit_template_insert', { content })
    message.success('已发送到编辑器窗口')
  } catch (e) {
    console.error('插入失败:', e)
    message.error('插入失败')
  } finally {
    loading.value = false
  }
}

const importFromFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Markdown', extensions: ['md', 'markdown'] }]
    })
    if (!selected) return

    const defaultName = String(selected).split(/[\\/]/).pop()?.replace(/\.(md|markdown)$/i, '') || '新模板'
    const name = window.prompt('模板名称', defaultName) || ''
    if (!name.trim()) return
    const type = window.prompt('模板类型（用于分类）', '未分类') || '未分类'

    loading.value = true
    await invoke('import_template_from_file', {
      sourcePath: selected,
      name: name.trim(),
      templateType: type.trim() || '未分类'
    })
    message.success('导入成功')
    await refresh()
  } catch (e) {
    console.error('导入失败:', e)
    message.error('导入失败')
  } finally {
    loading.value = false
  }
}

onMounted(refresh)
</script>

<style scoped>
.page {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 12px;
  box-sizing: border-box;
  gap: 10px;
  overflow: hidden;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Microsoft YaHei', system-ui, sans-serif;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.title {
  font-size: 16px;
  font-weight: 650;
  letter-spacing: 0.2px;
}

.path {
  font-size: 12px;
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding: 6px 10px;
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
}

.main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 360px 1fr;
  gap: 12px;
  overflow: hidden;
}

.left {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.left :deep(.n-collapse) {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.right {
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 12px;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 10px 28px rgba(16, 24, 40, 0.08);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.row-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.row-name-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.builtin-tag {
  flex: 0 0 auto;
  font-size: 11px;
  line-height: 18px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  opacity: 0.85;
}

.active {
  background: rgba(45, 212, 191, 0.12);
}

:global(:root[data-guaxi-theme='dark']) .active {
  background: rgba(97, 175, 239, 0.16);
}
</style>