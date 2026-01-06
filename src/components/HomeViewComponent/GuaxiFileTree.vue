<template>
  <div class="file-tree-container" ref="fileTreeContainerRef">
    <!-- 工具栏 -->
    <div class="list-header">
      <div class="title-container">
        <n-text class="repo-title-header">知识库编辑</n-text>
      </div>
      <div class="header-actions">
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper
              :size="18"
              :border-radius="6"
              class="browser-icon"
              :class="{ active: browserPreviewEnabled }"
              @click="toggleBrowserPreview"
            >
              <n-icon :component="GlobeOutline" size="16" />
            </n-icon-wrapper>
          </template>
          {{ browserPreviewEnabled ? '浏览器预览：已开启' : '浏览器预览：已关闭' }}
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper :size="18" :border-radius="6" class="refresh-icon" @click="refreshAll">
              <n-icon :component="RefreshIcon" size="16" />
            </n-icon-wrapper>
          </template>
          刷新所有目录
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper
              :size="18"
              :border-radius="6"
              class="kb-manager-icon"
              @click="openKnowledgeBaseManager"
            >
              <n-icon :component="SettingsOutline" size="16" />
            </n-icon-wrapper>
          </template>
          知识库管理器
        </n-tooltip>
      </div>
    </div>

    <div class="repo-body">
      <div v-if="activeRepoPathItem" class="active-repo-wrapper"
        :style="fileTreeStore.isRepoExpanded(activeRepoPathItem) ? { '--active-repo-max-height': activeRepoMaxHeight + 'px' } : undefined">
        <div class="repo-item" @click="toggleRepo(activeRepoPathItem)">
          <div class="repo-title">
            <n-icon size="15" class="repo-icon">
              <FolderIcon />
            </n-icon>
            {{ getRepoName(activeRepoPathItem) }}
          </div>
          <div class="repo-actions">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="20" :border-radius="4" class="action-icon" @click.stop="handleAddFile(activeRepoPathItem)">
                  <n-icon :component="FileAddIcon" size="14" />
                </n-icon-wrapper>
              </template>
              新建文件
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="20" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(activeRepoPathItem)">
                  <n-icon :component="FolderAddIcon" size="14" />
                </n-icon-wrapper>
              </template>
              新建文件夹
            </n-tooltip>
            <n-icon size="15" class="expand-icon">
              <chevron-down-icon v-if="fileTreeStore.isRepoExpanded(activeRepoPathItem)" />
              <chevron-forward-icon v-else />
            </n-icon>
          </div>
        </div>

        <n-collapse-transition>
          <div v-if="fileTreeStore.isRepoExpanded(activeRepoPathItem)" class="active-tree-container" @wheel.stop>
            <n-tree ref="treeRef" v-model:expanded-keys="expandedKeys" v-model:selected-keys="selectedKeys" :data="fileTree" key-field="id" :render-label="renderLabel"
              :render-prefix="renderPrefix" :expand-on-click="true" :show-line="true"
              @update:selected-keys="handleSelect" :node-props="nodeProps" virtual-scroll
              :virtual-scroll-item-height="28" class="tree-component" />
          </div>
        </n-collapse-transition>
      </div>

      <n-scrollbar ref="repoScrollbarRef" class="repo-scroll-container">
        <n-list bordered class="repo-list">
          <n-list-item v-for="path in inactiveRepoPaths" :key="path" class="repo-list-item">
            <div class="repo-item" @click="toggleRepo(path)">
              <div class="repo-title">
                <n-icon size="15" class="repo-icon">
                  <FolderIcon />
                </n-icon>
                {{ getRepoName(path) }}
              </div>
              <div class="repo-actions">
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="20" :border-radius="4" class="action-icon" @click.stop="handleAddFile(path)">
                      <n-icon :component="FileAddIcon" size="14" />
                    </n-icon-wrapper>
                  </template>
                  新建文件
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="20" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(path)">
                      <n-icon :component="FolderAddIcon" size="14" />
                    </n-icon-wrapper>
                  </template>
                  新建文件夹
                </n-tooltip>
                <n-icon size="15" class="expand-icon">
                  <chevron-forward-icon />
                </n-icon>
              </div>
            </div>
          </n-list-item>
        </n-list>
      </n-scrollbar>
    </div>

    <!-- 右键菜单 -->
    <n-dropdown v-model:show="isContextMenuVisible" :options="contextMenuOptions" placement="bottom-start"
      :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleContextMenuSelect"
      @clickoutside="handleClickoutside" />

    <n-modal v-model:show="importTemplateModalVisible" preset="dialog" title="导入到模板库" :mask-closable="false">
      <div class="import-template-modal-content">
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="模板名称">
            <n-input v-model:value="importTemplateName" placeholder="例如：会议纪要" />
          </n-form-item>
          <n-form-item label="模板类型">
            <n-input v-model:value="importTemplateType" placeholder="例如：会议 / 日记 / TODO" />
          </n-form-item>
          <n-form-item label="来源文件">
            <n-input :value="importTemplateSourcePath" disabled />
          </n-form-item>
        </n-form>
        <n-space justify="end">
          <n-button @click="importTemplateModalVisible = false">取消</n-button>
          <n-button type="primary" :loading="importTemplateLoading" @click="confirmImportAsTemplate">导入</n-button>
        </n-space>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { NTree, NIcon, useMessage, NModal, NForm, NFormItem, NInput, NButton, NSpace } from 'naive-ui'
import { DocumentTextOutline, FolderOutline, AddOutline, TrashOutline, CreateOutline, MoveOutline, FolderOpenOutline, GlobeOutline, SettingsOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { h, ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { listen } from '@tauri-apps/api/event'
import { NList, NListItem, NCollapseTransition, NText, NTooltip, NIconWrapper } from 'naive-ui'
import { RefreshOutline as RefreshIcon, ChevronDownOutline as ChevronDownIcon, ChevronForwardOutline as ChevronForwardIcon } from '@vicons/ionicons5'
import { Folder20Regular as FolderIcon, ChannelAdd20Regular as FileAddIcon, FolderAdd20Regular as FolderAddIcon } from '@vicons/fluent'


const message = useMessage();
// 处理文件树容器的右键点击事件
const treeRef = ref(null)

const BROWSER_PREVIEW_URL = 'http://localhost:8888/browser/knowledge-bases'
const browserPreviewEnabled = ref(false)

const toggleBrowserPreview = async () => {
  const next = !browserPreviewEnabled.value
  browserPreviewEnabled.value = next

  if (!next) return

  try {
    await openUrl(BROWSER_PREVIEW_URL)
  } catch (e) {
    browserPreviewEnabled.value = false
    message.error('打开浏览器失败，请确认已启用 opener 插件')
  }
}

const openKnowledgeBaseManager = async () => {
  try {
    await invoke('create_knowledge_base_manager_window', {
      label: 'knowledge_base_manager',
      title: '知识库管理器'
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('知识库管理器窗口已打开')
      return
    }
    console.error('打开知识库管理器失败:', err)
    message.error('打开知识库管理器失败')
  }
}

const isMarkdownFilePath = (p) => {
  const v = String(p || '').toLowerCase()
  return v.endsWith('.md') || v.endsWith('.markdown')
}

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const lastSelectedKey = ref(null);

const fileTreeContainerRef = ref(null)
const repoScrollbarRef = ref(null)
const activeRepoMaxHeight = ref(360)
const activeRepoResizeObserver = ref(null)

const activeRepoPathItem = computed(() => {
  const list = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  const active = fileTreeStore.activeRepoPath
  if (!active) return null

  const activeNorm = normalizePath(active)
  return list.find(p => normalizePath(p) === activeNorm) || null
})

const inactiveRepoPaths = computed(() => {
  const list = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  const active = activeRepoPathItem.value
  if (!active) return list

  const activeNorm = normalizePath(active)
  return list.filter(p => normalizePath(p) !== activeNorm)
})

const updateActiveRepoMaxHeight = () => {
  const el = fileTreeContainerRef.value
  if (!el) return

  const headerEl = el.querySelector('.list-header')
  const totalHeight = el.getBoundingClientRect().height
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0
  const scrollAreaHeight = totalHeight - headerHeight

  const reservedForInactive = 120
  activeRepoMaxHeight.value = Math.max(180, Math.floor(scrollAreaHeight - reservedForInactive))
}

watch(
  () => fileTreeStore.activeRepoPath,
  async () => {
    await nextTick()
    updateActiveRepoMaxHeight()
    repoScrollbarRef.value?.scrollTo?.({ top: 0 })
  }
)

// 文件树数据
const fileTree = ref([])
const isTreeLoading = ref(false)

// 右键菜单状态
const isContextMenuVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const selectedNode = ref(null) // 当前右键点击的节点

// 已展开的目录
const expandedKeys = ref([])
const unlistenRef = ref(null)

const selectedKeys = ref([]); // 新增：存储当前选中的节点keys

watch(
  () => fileTreeStore.openTabs.length,
  (len) => {
    if (len !== 0) return;
    lastSelectedKey.value = null;
    selectedKeys.value = [];
  }
)

const ensureExpandedForDir = (dirPath) => {
  const active = normalizePath(fileTreeStore.activeRepoPath)
  const target = normalizePath(dirPath)
  if (!target) return

  if (!active || !target.startsWith(active)) {
    if (!expandedKeys.value.includes(target)) expandedKeys.value.push(target)
    return
  }

  const rel = target.slice(active.length).replace(/^\/+/, '')
  if (!rel) return

  const parts = rel.split('/').filter(Boolean)
  let acc = active
  for (const part of parts) {
    acc = `${acc}/${part}`
    if (!expandedKeys.value.includes(acc)) expandedKeys.value.push(acc)
  }
}

const scrollToKey = async (key) => {
  await nextTick()
  await new Promise((r) => requestAnimationFrame(r))

  const tree = treeRef.value
  if (!tree || typeof tree.scrollTo !== 'function') return

  try {
    await Promise.resolve(tree.scrollTo({ key }))
  } catch {
    return
  }
}

const safeCssEscape = (v) => {
  const s = String(v ?? '')
  try {
    if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(s)
  } catch {
    return s.replace(/"/g, '\\"')
  }
  return s.replace(/"/g, '\\"')
}

const focusEditingInput = async (nodeId) => {
  const id = normalizePath(nodeId)
  for (let i = 0; i < 8; i += 1) {
    await nextTick()
    await new Promise((r) => requestAnimationFrame(r))

    const selector = `.editable-input[data-node-id="${safeCssEscape(id)}"]`
    const root = fileTreeContainerRef.value
    const input = (root && root.querySelector(selector)) || document.querySelector(selector)
    if (input) {
      input.focus()
      input.select()
      return true
    }
  }
  return false
}

const importTemplateModalVisible = ref(false)
const importTemplateLoading = ref(false)
const importTemplateName = ref('')
const importTemplateType = ref('未分类')
const importTemplateSourcePath = ref('')

const basename = (p) => String(p || '').split(/[\\/]/).pop() || ''
const stemname = (p) => {
  const b = basename(p)
  const idx = b.lastIndexOf('.')
  return idx > 0 ? b.slice(0, idx) : b
}

const openImportTemplateModal = async () => {
  const node = selectedNode.value
  if (!node || node.is_directory) return

  const nodePath = normalizePath(node.id)
  if (!isMarkdownFilePath(nodePath)) {
    message.warning('仅支持 Markdown 文件')
    return
  }

  importTemplateSourcePath.value = nodePath
  importTemplateName.value = stemname(nodePath)
  importTemplateType.value = '未分类'
  importTemplateModalVisible.value = true
}

const confirmImportAsTemplate = async () => {
  const sourcePath = importTemplateSourcePath.value
  const name = importTemplateName.value.trim()
  const templateType = importTemplateType.value.trim() || '未分类'

  if (!sourcePath) return
  if (!name) {
    message.warning('请输入模板名称')
    return
  }

  try {
    importTemplateLoading.value = true
    await invoke('import_template_from_file', {
      sourcePath,
      name,
      templateType
    })
    message.success('已导入到模板库')
    importTemplateModalVisible.value = false
  } catch (error) {
    console.error('导入模板失败:', error)
    message.error('导入模板失败')
  } finally {
    importTemplateLoading.value = false
  }
}

// 右键菜单选项
const contextMenuOptions = ref([
  {
    label: '新建文件',
    key: 'addFile',
    icon: () => h(NIcon, null, { default: () => h(AddOutline) })
  },
  {
    label: '新建文件夹',
    key: 'addFolder',
    icon: () => h(NIcon, null, { default: () => h(FolderOpenOutline) })
  },
  {
    label: '导入到模板库',
    key: 'importAsTemplate',
    disabled: true,
    icon: () => h(NIcon, null, { default: () => h(DocumentTextOutline) })
  },
  {
    label: '重命名',
    key: 'rename',
    icon: () => h(NIcon, null, { default: () => h(CreateOutline) })
  },
  {
    label: '删除',
    key: 'delete',
    icon: () => h(NIcon, null, { default: () => h(TrashOutline) })
  },
  {
    label: '移动到',
    key: 'move',
    icon: () => h(NIcon, null, { default: () => h(MoveOutline) })
  },
  {
    label: '在文件浏览器中打开',
    key: 'openInExplorer',
    icon: () => h(NIcon, null, { default: () => h(FolderOutline) })
  },
  {
    label: '在浏览器中查看',
    key: 'openInBrowser',
    disabled: true,
    icon: () => h(NIcon, null, { default: () => h(GlobeOutline) })
  },
])

const isHiddenPath = (path) => {
  if (!path) return false;
  const normalized = normalizePath(path);
  return normalized.split('/').some(part => part.startsWith('.'));
};

// 修改后的handleAddFile函数
const handleAddFile = async (repoPath) => {
  try {
    let parentPath = repoPath; // 默认使用仓库路径

    // 如果有选中的节点，获取其路径
    if (selectedKeys.value.length > 0) {
      const selectedKey = selectedKeys.value[0];
      const selectedNode = findNodeById(fileTree.value, selectedKey);

      // 如果选中的是目录，则使用该目录路径
      if (selectedNode?.is_directory) {
        parentPath = selectedNode.id;
      }
    }

    await addNode(parentPath, false);
  } catch (error) {
    message.error(`添加文件失败: ${error.message}`);
  }
};

// 修改后的handleAddFolder函数
const handleAddFolder = async (repoPath) => {
  try {
    let parentPath = repoPath; // 默认使用仓库路径

    // 如果有选中的节点，获取其路径
    if (selectedKeys.value.length > 0) {
      const selectedKey = selectedKeys.value[0];
      const selectedNode = findNodeById(fileTree.value, selectedKey);

      // 如果选中的是目录，则使用该目录路径
      if (selectedNode?.is_directory) {
        parentPath = selectedNode.id;
      }
    }

    await addNode(parentPath, true);
  } catch (error) {
    message.error(`添加文件夹失败: ${error.message}`);
  }
};

// 获取仓库显示名称
const getRepoName = (path) => {
  const name = path.split(/[\\/]/).pop() || path
  // console.log('[Util] 获取仓库名称:', { path, name })
  return name
}

const toggleRepo = async (path) => {
  // console.log('目标路径:', path)
  // console.log('当前激活路径:', fileTreeStore.activeRepoPath)

  if (fileTreeStore.activeRepoPath === path) {
    // 如果点击的是当前已激活的仓库，则切换展开状态
    fileTreeStore.toggleRepoExpand(path)
  } else {
    // 如果是新仓库，则设置激活并展开
    fileTreeStore.setActiveRepoPath(path)
    fileTreeStore.expandedRepos.set(path, true) // 确保新仓库展开
    await loadRepoTree(path)
  }
}

const loadRepoTree = async (path) => {
  // console.log('[API] 加载仓库树')
  try {
    // 无论数据是否为空都重新加载
    await loadFileTree()
    // 确保仓库在加载后保持展开状态
    // fileTreeStore.expandedRepos.set(path, false)
  } catch (error) {
    console.error('[API] 加载仓库树失败:', error)
  }
}

const refreshAll = async () => {
  const activePath = fileTreeStore.activeRepoPath
  if (!activePath) return
  console.log('[Refresh] 刷新仓库:', activePath)
  await loadRepoTree(activePath)
}

const normalizeTreeIds = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];
  return tree.map(item => {
    const newItem = { ...item, id: normalizePath(item.id) };
    if (item.children) {
      newItem.children = normalizeTreeIds(item.children);
    }
    return newItem;
  });
};

// 加载文件树数据
const loadFileTree = async (options = {}) => {
  const { force = false } = options || {}
  console.log('[API] 加载文件树');
  isTreeLoading.value = true
  try {
    console.log('当前仓库路径:', fileTreeStore.activeRepoPath);
    await fileTreeStore.fetchFileTree(fileTreeStore.activeRepoPath, { force });

    const filteredTree = filterHiddenTree(fileTreeStore.fileTree);

    fileTree.value = [...sortFileTree(normalizeTreeIds(filteredTree))];
  } catch (error) {
    console.error('获取文件树数据失败', error);
    fileTree.value = [];
  } finally {
    isTreeLoading.value = false
  }
};

const filterHiddenTree = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];
  return tree
    .filter(item => !isHiddenPath(item.id))
    .map(item => {
      const next = { ...item };
      if (item.children) {
        next.children = filterHiddenTree(item.children);
      }
      return next;
    });
};

const treeNameCollator = (() => {
  try {
    if (typeof Intl !== 'undefined' && typeof Intl.Collator === 'function') {
      return new Intl.Collator('zh-u-co-pinyin', { numeric: true, sensitivity: 'base' })
    }
  } catch {
    return null
  }
  return null
})()

// 对文件树数据进行排序（目录在上，文件在下；英文按字母，中文按拼音）
const sortFileTree = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];

  const compareName = (aName, bName) => {
    const aStr = String(aName || '')
    const bStr = String(bName || '')
    if (treeNameCollator) return treeNameCollator.compare(aStr, bStr)
    return aStr.localeCompare(bStr)
  }

  return tree
    .map(item => {
      if (item.children) {
        item.children = sortFileTree(item.children);
      }
      return item;
    })
    .sort((a, b) => {
      const aIsDir = !!a.is_directory || a.type === 'directory'
      const bIsDir = !!b.is_directory || b.type === 'directory'
      if (aIsDir !== bIsDir) return aIsDir ? -1 : 1
      return compareName(a.name, b.name)
    });
};

// 渲染文件树图标
const renderPrefix = ({ option }) => {
  const icon = option.is_directory ? FolderOutline : DocumentTextOutline
  return h(NIcon, null, { default: () => h(icon) })
}

const handleSelect = async (keys) => {
  console.log('触发选中事件, keys 参数:', keys);

  // 1. 处理空选中情况
  if (!keys || keys.length === 0) {
    if (isTreeLoading.value) {
      console.log('空选中（加载中），忽略本次清空事件');
      return;
    }
    console.log('空选中，清空选中状态');
    lastSelectedKey.value = null;
    selectedKeys.value = [];
    return;
  }

  const selectedKey = keys[0];
  const normalizedKey = normalizePath(selectedKey);

  // 检查是否是隐藏路径
  if (isHiddenPath(normalizedKey)) {
    console.log('跳过隐藏目录/文件:', normalizedKey);
    return;
  }

  // 2. 防抖处理 - 仅当已存在标签页时忽略重复选中
  if (normalizedKey === normalizePath(lastSelectedKey.value)) {
    const existingTab = fileTreeStore.openTabs.find(tab => normalizePath(tab.path) === normalizedKey)
    if (existingTab) {
      console.log('忽略重复选中（已打开标签页）:', normalizedKey);
      fileTreeStore.setActiveTab(existingTab.path)
      return;
    }
  }

  // 3. 更新选中状态
  selectedKeys.value = [selectedKey];
  lastSelectedKey.value = selectedKey;

  console.log('查找选中节点, key:', normalizedKey);
  const selectedNode = findNodeById(fileTree.value, normalizedKey);

  if (!selectedNode) {
    console.warn('未找到选中节点');
    lastSelectedKey.value = null;
    selectedKeys.value = [];
    return;
  }

  console.log('找到选中节点:', selectedNode);

  if (editingNode.value || selectedNode.isNew) {
    lastSelectedKey.value = null
    return
  }

  if (!selectedNode.is_directory) {
    try {
      // 检查是否已存在标签页
      const existingTab = fileTreeStore.openTabs.find(tab => tab.path === selectedNode.id);
      if (existingTab) {
        console.log('文件已在标签页中打开');
        fileTreeStore.setActiveTab(selectedNode.id);
        return;
      }

      // 获取文件内容
      const res = await fileTreeStore.fetchFileContent(selectedNode.id)
      if (res?.ok) {
        await fileTreeStore.openFileInTab(selectedNode.id, res.content, {
          unsupported: !!res.unsupported
        })
        if (res.unsupported) {
          message.warning('不支持打开该文件类型（仅支持文本/Markdown）')
        }
      } else {
        message.warning('文件不存在或已被删除')
        await handleFileRemoved(selectedNode.id)
      }
    } catch (error) {
      console.error('处理文件选中时出错:', error);
    }
  } else {
    try {
      const dirId = normalizePath(selectedNode.id)

      await fileTreeStore.fetchFileTree(dirId, { force: true });
      const filteredChildren = filterHiddenTree(fileTreeStore.fileTree);
      selectedNode.children = sortFileTree(normalizeTreeIds(filteredChildren));

      if (!expandedKeys.value.includes(dirId) && Array.isArray(selectedNode.children) && selectedNode.children.length > 0) {
        ensureExpandedForDir(dirId)
      }

      await scrollToKey(dirId)
    } catch (error) {
      console.error('加载子目录内容失败:', error);
    }
  }
};

// 递归查找节点
const findNodeById = (tree, targetId) => {
  const normalizedTargetId = normalizePath(targetId);
  console.log('查找节点:', normalizedTargetId);

  // 跳过隐藏目录
  if (isHiddenPath(normalizedTargetId)) {
    console.log('跳过隐藏目录:', normalizedTargetId);
    return null;
  }

  // 深度优先搜索
  const dfs = (nodes) => {
    for (const node of nodes) {
      const nodePath = normalizePath(node.id);
      
      // 跳过隐藏目录
      if (isHiddenPath(nodePath)) {
        continue;
      }

      if (nodePath === normalizedTargetId) {
        return node;
      }

      if (node.children) {
        const found = dfs(node.children);
        if (found) return found;
      }
    }
    return null;
  };

  return dfs(tree);
};

const nodeProps = ({ option }) => ({
  // 处理右键点击
  onContextmenu: (e) => {
    e.preventDefault()
    e.stopPropagation()
    selectedNode.value = option

    const nodePath = normalizePath(option.id)
    const canOpenInBrowser = !option.is_directory && isMarkdownFilePath(nodePath)
    const openInBrowserOpt = contextMenuOptions.value.find(o => o.key === 'openInBrowser')
    if (openInBrowserOpt) {
      openInBrowserOpt.disabled = !canOpenInBrowser
    }

    const canImportAsTemplate = !option.is_directory && isMarkdownFilePath(nodePath)
    const importOpt = contextMenuOptions.value.find(o => o.key === 'importAsTemplate')
    if (importOpt) {
      importOpt.disabled = !canImportAsTemplate
    }

    isContextMenuVisible.value = true
    contextMenuPosition.value = { x: e.clientX, y: e.clientY }
    console.log('节点右键菜单触发:', option.name)
  }
})

// 处理右键菜单选择
const handleContextMenuSelect = async (key) => {
  isContextMenuVisible.value = false;
  try {
    switch (key) {
      case 'addFile':
        await addNode(selectedNode.value?.id, false);
        break;
      case 'addFolder':
        await addNode(selectedNode.value?.id, true);
        break;
      case 'importAsTemplate':
        await openImportTemplateModal();
        break;
      case 'rename':
        if (selectedNode.value) {
          renameNode(selectedNode.value.id);
        }
        break;
      case 'delete':
        if (selectedNode.value) {
          deleteNode(selectedNode.value.id);
        }
        break;
      case 'move':
        if (selectedNode.value) {
          moveNode(selectedNode.value);
        }
        break;
      case 'openInExplorer':
        if (selectedNode.value) {
          const nodePath = normalizePath(selectedNode.value.id);
          const targetPath = selectedNode.value.is_directory
            ? nodePath
            : nodePath.split('/').slice(0, -1).join('/');
          if (targetPath) {
            await invoke('open_in_file_explorer', { path: targetPath });
          }
        }
        break;
      case 'openInBrowser':
        if (selectedNode.value && !selectedNode.value.is_directory) {
          const nodePath = normalizePath(selectedNode.value.id)
          if (!isMarkdownFilePath(nodePath)) {
            message.warning('仅支持 Markdown 文件')
            break
          }
          const url = `http://localhost:8888/browser/preview?path=${encodeURIComponent(nodePath)}`
          await openUrl(url)
        }
        break;
    }
  } catch (error) {
    message.error(`操作失败: ${error.message}`);
  }
};

// 处理点击外部关闭菜单
const handleClickoutside = () => {
  isContextMenuVisible.value = false
}

// 添加一个 ref 来存储当前正在编辑的节点
const editingNode = ref(null)
// 添加一个 ref 来存储编辑中的名称
const editName = ref('')

// 修改后的 addNode 函数
const addNode = async (parentId, isDirectory = false) => {
  parentId = normalizePath(parentId);

  const activeRepo = normalizePath(fileTreeStore.activeRepoPath)
  const isRoot = !parentId || parentId === activeRepo

  let parentNode = null
  if (!isRoot) {
    parentNode = findNodeById(fileTree.value, parentId);
    if (!parentNode) {
      parentId = activeRepo
      parentNode = null
    }
  }

  if (!isRoot && parentNode?.is_directory) {
    ensureExpandedForDir(parentNode.id)
    if (!Array.isArray(parentNode.children) || parentNode.children.length === 0) {
      await fileTreeStore.fetchFileTree(parentNode.id)
      const filteredChildren = filterHiddenTree(fileTreeStore.fileTree)
      parentNode.children = sortFileTree(normalizeTreeIds(filteredChildren))
    }
  }

  const tempName = isDirectory ? '新建文件夹' : '新建文件';
  const baseDir = isRoot ? activeRepo : normalizePath(parentNode?.id)
  const tempId = `${baseDir}/${tempName}`;

  const newNode = {
    id: tempId,
    name: tempName,
    is_directory: isDirectory,
    children: isDirectory ? [] : undefined,
    isNew: true
  };

  if (isRoot) {
    fileTree.value = [newNode, ...fileTree.value];
  } else {
    parentNode.children = [newNode, ...(parentNode.children || [])];
  }

  if (!isRoot && parentNode?.id) {
    ensureExpandedForDir(parentNode.id)
  }

  startEditing(newNode);

  await nextTick()
  await scrollToKey(newNode.id)
  await focusEditingInput(newNode.id)

  selectedKeys.value = [];

  return new Promise((resolve) => {
    const unwatch = watch(editingNode, (newVal) => {
      if (!newVal) {
        unwatch();
        handleEditComplete(newNode, parentNode, isDirectory, isRoot, resolve);
      }
    });
  });
};

// 处理编辑完成的函数
const handleEditComplete = async (node, parentNode, isDirectory, isRoot, resolve) => {
  const removeTempNode = () => {
    if (isRoot) {
      fileTree.value = fileTree.value.filter(n => n.id !== node.id)
      return
    }

    if (parentNode?.children) {
      parentNode.children = parentNode.children.filter(n => n.id !== node.id)
    }
  }

  if (node.name === '新建文件夹' || node.name === '新建文件' || !node.name.trim()) {
    removeTempNode()
    resolve(null);
    return;
  }

  try {
    const targetPath = normalizePath(node.id)
    node.id = targetPath

    if (isDirectory) {
      await invoke('create_directory', { path: targetPath })
    } else {
      await invoke('create_file_with_content', {
        path: targetPath,
        content: ''
      })
    }

    delete node.isNew

    const parentDir = isRoot
      ? fileTreeStore.activeRepoPath
      : parentNode?.id

    if (parentDir) {
      await refreshDirectory(parentDir)
    }

    resolve(node);
  } catch (error) {
    console.error('创建失败:', error);
    removeTempNode()
    message.error(`创建失败: ${error.message}`);
    resolve(null);
  } finally {
    selectedKeys.value = [];
  }
};

// 修改后的 handleBlur 函数
const handleBlur = async () => {
  if (!editingNode.value) return;

  const node = editingNode.value;
  const oldId = node.id;
  const parentNode = findParentNode(fileTree.value, oldId);
  let parentPath = parentNode ? parentNode.id : fileTreeStore.activeRepoPath;

  if (!editName.value.trim() ||
    editName.value === '新建文件' ||
    editName.value === '新建文件夹') {
    // 名称为空，如果是新节点则删除
    if (node.isNew) {
      if (!parentNode) {
        // console.warn('未找到父节点，无法删除新节点');
        // 根目录下直接从 fileTree.value 里移除
        fileTree.value = fileTree.value.filter(n => n.id !== oldId);
        // 新增：刷新根目录
        parentPath = normalizePath(node.id).split('/').slice(0, -1).join('/');
        if (!parentPath || parentPath === normalizePath(fileTreeStore.activeRepoPath)) {
          await loadFileTree();
        } else {
          await refreshDirectory(parentPath);
        }
      } else {
        parentNode.children = parentNode.children.filter(n => n.id !== oldId);
        if (normalizePath(parentPath) === normalizePath(fileTreeStore.activeRepoPath)) {
          await loadFileTree();
        } else {
          await refreshDirectory(parentPath);
        }
      }
    }
    editingNode.value = null;
    editName.value = '';
    return;
  }

  // 构建新路径（统一规范化，避免 Windows 路径分隔符导致创建/读取不一致）
  const basePath = normalizePath(parentPath)
  const newId = normalizePath(`${basePath}/${editName.value}`)

  // 更新节点信息
  node.name = editName.value
  node.id = newId

  // 如果是新节点，结束编辑（实际文件会在handleEditComplete中创建）
  if (node.isNew) {
    editingNode.value = null;
    editName.value = '';
    return;
  }

  // 以下是原有重命名逻辑
  try {
    if (node.is_directory) {
      await invoke('rename_directory', { oldPath: oldId, newPath: newId });
    } else {
      await invoke('rename_path', { oldPath: oldId, newPath: newId });
    }

    // 更新标签页
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === oldId);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].path = newId;
      fileTreeStore.openTabs[tabIndex].fileName = editName.value;
      // 同步内容（如果内容有变化）
      fileTreeStore.openTabs[tabIndex].content = node.content || fileTreeStore.openTabs[tabIndex].content;
      if (fileTreeStore.activeTab === oldId) {
        fileTreeStore.setActiveTab(newId);
      }
    }
  } catch (error) {
    console.error('重命名操作失败:', error);
    node.name = oldId.split(/[\\/]/).pop();
    node.id = oldId;
    message.error(`重命名失败: ${error.message}`);
  } finally {
    editingNode.value = null;
    editName.value = '';
    // 根目录下刷新整个树，否则刷新父目录
    if (normalizePath(parentPath) === normalizePath(fileTreeStore.activeRepoPath)) {
      await loadFileTree({ force: true });
    } else {
      await refreshDirectory(parentPath);
    }
  }
};

// 删除文件或目录
const deleteNode = async (nodeId) => {
  const normalizedId = normalizePath(nodeId)
  const node = findNodeById(fileTree.value, normalizedId)
  if (!node) return

  const ok = await confirmDialog(`确定要删除 ${node.name} 吗？`, {
    title: '删除',
    kind: 'warning'
  })
  if (!ok) return

  try {
    await invoke('delete_path', { path: normalizedId })
    await handleFileRemoved(normalizedId)
  } catch (error) {
    message.error(`删除失败: ${error.message}`)
    const parentPath = normalizedId.split('/').slice(0, -1).join('/') || normalizePath(fileTreeStore.activeRepoPath)
    await refreshDirectory(parentPath)
  } finally {
    selectedKeys.value = []
    lastSelectedKey.value = null
  }
}

// 重命名文件或目录
const renameNode = async (nodeId) => {
  const node = findNodeById(fileTree.value, nodeId)
  if (node) {
    // 设置节点为编辑状态
    startEditing(node)
  }
}

// 修改后的 findParentNode 函数
const findParentNode = (tree, targetId) => {
  const normalizedTargetId = normalizePath(targetId);
  console.log('查找父节点:', normalizedTargetId);

  // 深度优先搜索
  const dfs = (nodes, parent) => {
    for (const node of nodes) {
      const nodePath = normalizePath(node.id);

      if (nodePath === normalizedTargetId) {
        return parent;
      }

      if (node.children) {
        const found = dfs(node.children, node);
        if (found) return found;
      }
    }
    return null;
  };

  return dfs(tree, null);
};

// 开始编辑节点
const startEditing = (node) => {
  console.log('startEditing 开始编辑节点:', node.name);
  editingNode.value = node
  editName.value = node.name
  console.log('startEditing ==> ', node);
}

const renderLabel = ({ option }) => {
  // 用 id 判断是否为正在编辑的节点
  if (editingNode.value && normalizePath(editingNode.value.id) === normalizePath(option.id)) {
    console.log('EEE ======> renderLabel editingNode.value:', editingNode.value);
    return h('input', {
      class: 'editable-input',
      'data-node-id': normalizePath(option.id),
      value: editName.value,
      onInput: (e) => (editName.value = e.target.value),
      onBlur: handleBlur,
      onKeydown: handleKeydown,
    })
  } else {
    // console.log('======> renderLabel option:', option);
    // console.log('======> renderLabel editingNode.value:', editingNode.value);
    return option.name
  }
}

watch(editingNode, async (val) => {
  if (!val) return
  await focusEditingInput(val.id)
})

// 处理输入框的 keydown 事件
const handleKeydown = async (e) => {
  console.log('handleKeydown triggered'); // 添加日志
  if (e.key === 'Enter') {
    e.target.blur(); // 触发 input 失焦事件
  } else if (e.key === 'Escape') {
    // 取消编辑
    if (editingNode.value && editingNode.value.isNew) {
      const node = editingNode.value;
      let parentNode = findParentNode(fileTree.value, node.id);
      let parentPath = parentNode ? parentNode.id : null;

      // 新增：如果 parentNode 没找到，直接用路径推断父目录
      if (!parentNode) {
        parentPath = normalizePath(node.id).split('/').slice(0, -1).join('/');
        // 根目录特殊处理
        if (!parentPath || parentPath === normalizePath(fileTreeStore.activeRepoPath)) {
          await loadFileTree();
        } else {
          await refreshDirectory(parentPath);
        }
      } else {
        parentNode.children = parentNode.children.filter(n => n.id !== node.id);
        // 根目录下刷新整个树，否则刷新父目录
        if (normalizePath(parentPath) === normalizePath(fileTreeStore.activeRepoPath)) {
          await loadFileTree();
        } else {
          await refreshDirectory(parentPath);
        }
      }
    }
    editingNode.value = null;
    editName.value = '';
  }
};

const moveNode = async (node) => {
  const targetDirectoryPath = await selectTargetPath();
  if (!targetDirectoryPath) return;

  const sourcePath = normalizePath(node.id);
  const targetDir = normalizePath(targetDirectoryPath);
  const destinationPath = normalizePath(`${targetDir}/${node.name}`);
  const sourceParentPath = sourcePath.split('/').slice(0, -1).join('/') || normalizePath(fileTreeStore.activeRepoPath);

  try {
    console.log('源路径:', sourcePath);
    console.log('目标路径:', destinationPath);

    await invoke('move_path', { sourcePath, destinationPath });

    const parentNode = findParentNode(fileTree.value, sourcePath);
    if (parentNode?.children?.length) {
      parentNode.children = parentNode.children.filter(
        (child) => normalizePath(child.id) !== sourcePath
      );
    } else {
      fileTree.value = fileTree.value.filter((n) => normalizePath(n.id) !== sourcePath);
    }

    const targetNode = findNodeById(fileTree.value, targetDir);
    if (targetNode && targetNode.is_directory) {
      if (!Array.isArray(targetNode.children)) targetNode.children = [];
      node.id = destinationPath;
      targetNode.children.push(node);
    } else {
      node.id = destinationPath;
      await loadFileTree({ force: true });
    }

    const tabIndex = fileTreeStore.openTabs.findIndex((tab) => normalizePath(tab.path) === sourcePath);
    if (tabIndex !== -1) {
      const tab = fileTreeStore.openTabs[tabIndex];
      tab.path = destinationPath;
      tab.fileName = destinationPath.split('/').pop();
      if (fileTreeStore.activeTab && normalizePath(fileTreeStore.activeTab) === sourcePath) {
        fileTreeStore.setActiveTab(destinationPath);
      }
    }

    if (selectedKeys.value?.length && normalizePath(selectedKeys.value[0]) === sourcePath) {
      selectedKeys.value = [destinationPath];
      lastSelectedKey.value = destinationPath;
    }

    await refreshDirectory(sourceParentPath);
    await refreshDirectory(targetDir);

    console.log('移动成功');
  } catch (error) {
    console.error('移动失败:', error);
    await refreshDirectory(sourceParentPath);
    await refreshDirectory(targetDir);
  }
};

const selectTargetPath = async () => {
  const result = await open({
    directory: true,
    multiple: false,
    title: '选择目标路径',
  });

  if (result) {
    return result;
  } else {
    return null;
  }
};

const refreshDirectory = async (directoryId) => {
  const activeRepo = normalizePath(fileTreeStore.activeRepoPath)
  const normalizedId = normalizePath(directoryId)
  console.log('刷新目录:', normalizedId)

  if (isHiddenPath(normalizedId)) {
    console.log('跳过隐藏目录刷新:', normalizedId)
    return
  }

  if (!normalizedId || !activeRepo || normalizedId === activeRepo) {
    await loadFileTree({ force: true })
    return
  }

  let targetDir = normalizedId

  const maybeNode = findNodeById(fileTree.value, targetDir)
  if (maybeNode && !maybeNode.is_directory) {
    targetDir = targetDir.split('/').slice(0, -1).join('/') || activeRepo
  }

  if (!targetDir || targetDir === activeRepo) {
    await loadFileTree({ force: true })
    return
  }

  const directoryNode = findNodeById(fileTree.value, targetDir)
  if (!directoryNode || !directoryNode.is_directory) {
    await loadFileTree({ force: true })
    return
  }

  try {
    const wasExpanded = expandedKeys.value.includes(targetDir)

    await fileTreeStore.fetchFileTree(targetDir, { force: true })

    const filteredTree = filterHiddenTree(fileTreeStore.fileTree)
    const sortedChildren = sortFileTree(normalizeTreeIds(filteredTree))

    const newChildren = sortedChildren.map(newNode => {
      const existingNode = findNodeById([directoryNode], newNode.id)
      return existingNode || newNode
    })

    directoryNode.children = [...newChildren]

    if (wasExpanded && !expandedKeys.value.includes(targetDir)) {
      expandedKeys.value.push(targetDir)
    }

    console.log('目录局部刷新成功:', directoryNode)
  } catch (error) {
    console.error('目录刷新失败:', error)
  }
};

// 更健壮的路径规范化函数
const normalizePath = (path) => {
  if (!path) return '';

  // 替换所有反斜杠为正斜杠
  let normalized = path.replace(/\\/g, '/');

  // 处理Windows盘符 (如 C:/)
  if (normalized.match(/^[a-zA-Z]:\//)) {
    normalized = normalized[0].toUpperCase() + normalized.slice(1);
  }

  // 移除末尾斜杠（目录除外）
  if (normalized.endsWith('/') && normalized.length > 1) {
    normalized = normalized.slice(0, -1);
  }

  return normalized;
};

// 处理文件创建事件
const handleFileCreated = async (filePath) => {
  const normalizedPath = normalizePath(filePath);
  const parentPath = normalizedPath.split('/').slice(0, -1).join('/');

  // 检查是否是隐藏路径
  if (isHiddenPath(normalizedPath) || isHiddenPath(parentPath)) {
    console.log('跳过隐藏目录的文件创建事件:', normalizedPath);
    return;
  }

  await refreshDirectory(parentPath || fileTreeStore.activeRepoPath);
};

// 处理文件重命名事件
const handleFileRenamed = async (oldPath, newPath) => {
  if (!newPath) return;

  try {
    const normalizedNewPath = normalizePath(newPath);
    const normalizedOldPath = oldPath ? normalizePath(oldPath) : null;

    // 检查是否是隐藏路径
    if (isHiddenPath(normalizedNewPath) || (normalizedOldPath && isHiddenPath(normalizedOldPath))) {
      console.log('跳过隐藏目录的文件重命名事件:', normalizedOldPath, '->', normalizedNewPath);
      return;
    }

    // 1. 更新树中的节点
    if (normalizedOldPath) {
      const nodeToUpdate = findNodeById(fileTree.value, normalizedOldPath);
      if (nodeToUpdate) {
        nodeToUpdate.id = normalizedNewPath;
        nodeToUpdate.name = newPath.split(/[\\/]/).pop();
      }
    }

    // 2. 更新标签页
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === normalizedOldPath);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].path = normalizedNewPath;
      fileTreeStore.openTabs[tabIndex].fileName = newPath.split(/[\\/]/).pop();

      if (fileTreeStore.activeTab === normalizedOldPath) {
        fileTreeStore.setActiveTab(normalizedNewPath);
      }
    }

    // 3. 刷新父目录
    const parentPath = normalizedOldPath
      ? normalizedOldPath.split('/').slice(0, -1).join('/')
      : normalizedNewPath.split('/').slice(0, -1).join('/');

    await refreshDirectory(parentPath || fileTreeStore.activeRepoPath);
  } catch (error) {
    console.error('处理文件重命名时出错:', error);
  }
};

// 文件监控函数
const setupFileWatcher = async () => {
  try {
    console.log('开始监控目录:', fileTreeStore.activeRepoPath)

    // 启动后端监控
    await invoke('start_file_watching', {
      path: fileTreeStore.activeRepoPath
    })

    // 监听文件变化事件
    if (unlistenRef.value) {
      // 先移除旧监听器，避免重复
      await unlistenRef.value();
      unlistenRef.value = null;
    }

    // 监听文件变化事件
    const unlisten = await listen('file_change', (event) => {
      const payload = event.payload
      console.log('文件系统变化事件:', payload)

      switch (payload.kind) {
        case 'Create':
          // console.log('文件创建事件:', payload.path)
          handleFileCreated(payload.path)
          break
        case 'Remove':
          // console.log('文件删除事件:', payload.path)
          handleFileRemoved(payload.path)
          break
        case 'Rename':
          // 处理old_path可能为null的情况
          const oldPath = payload.old_path || extractOldPathFromEvent(payload)
          // console.log('文件重命名事件:', oldPath, '->', payload.path)
          handleFileRenamed(oldPath, payload.path)
          break
      }
    })
    unlistenRef.value = unlisten; // ★ 保存监听器

    console.log('文件监控已启动')
  } catch (error) {
    console.error('启动文件监控失败:', error)
  }
}

// 辅助函数：从事件中提取旧路径（如果需要）
const extractOldPathFromEvent = (event) => {
  // 根据实际情况实现从事件中提取旧路径的逻辑
  return null // 如果无法提取则返回null
}

// 优化处理文件删除事件的函数
const handleFileRemoved = async (filePath) => {
  const removedPath = normalizePath(filePath)
  const parentPath = removedPath.split('/').slice(0, -1).join('/')

  if (isHiddenPath(removedPath) || isHiddenPath(parentPath)) {
    console.log('跳过隐藏目录的文件删除事件:', removedPath)
    return
  }

  const removedNode = findNodeById(fileTree.value, removedPath)
  const isDir =
    !!removedNode?.is_directory ||
    fileTreeStore.openTabs.some((tab) => normalizePath(tab.path).startsWith(`${removedPath}/`))

  const removeFromTree = (nodes) => {
    if (!Array.isArray(nodes)) return []
    return nodes
      .filter((node) => normalizePath(node.id) !== removedPath)
      .map((node) => {
        if (node.children) node.children = removeFromTree(node.children)
        return node
      })
  }

  fileTree.value = removeFromTree(fileTree.value)

  if (selectedKeys.value?.length && normalizePath(selectedKeys.value[0]) === removedPath) {
    selectedKeys.value = []
    lastSelectedKey.value = null
  }

  if (expandedKeys.value?.length) {
    expandedKeys.value = expandedKeys.value.filter((key) => {
      const k = normalizePath(key)
      if (k === removedPath) return false
      if (isDir && k.startsWith(`${removedPath}/`)) return false
      return true
    })
  }

  const tabPathsToClose = fileTreeStore.openTabs
    .map((tab) => tab.path)
    .filter((p) => {
      const np = normalizePath(p)
      if (np === removedPath) return true
      return isDir && np.startsWith(`${removedPath}/`)
    })

  for (const p of tabPathsToClose) {
    fileTreeStore.closeTab(p)
  }

  await refreshDirectory(parentPath || fileTreeStore.activeRepoPath)
};

// 清理函数
const cleanupFileWatcher = async () => {
  try {
    await invoke('stop_file_watching')

    if (unlistenRef.value && typeof unlistenRef.value === 'function') {
      await unlistenRef.value()
      unlistenRef.value = null
    }
    console.log('文件监控已停止')
  } catch (error) {
    if (error && error.message && error.message.includes('unregisterListener')) {
      console.warn('文件监控已经停止，无需重复停止')
      return
    }
    console.error('停止文件监控失败:', error)
  }
}

const syncRepoPathsFromKnowledgeBases = (knowledgeBases) => {
  const list = Array.isArray(knowledgeBases) ? knowledgeBases : []
  const paths = list.map(kb => kb.location)
  fileTreeStore.setRepopathlist(paths)

  const currentActive = fileTreeStore.activeRepoPath
  const defaultActive = paths.length > 0 ? paths[0] : null

  if (!currentActive && defaultActive) {
    fileTreeStore.setActiveRepoPath(defaultActive)
  } else if (currentActive && !paths.includes(currentActive)) {
    fileTreeStore.setActiveRepoPath(defaultActive)
  }

  return paths
}

// 新增初始化函数
const initializeRepositories = async () => {
  console.log('[Init] 开始初始化知识库')

  try {
    let knowledgeBases = knowledgeBaseStore.knowledgeBases
    if (!knowledgeBases || knowledgeBases.length === 0) {
      knowledgeBases = await knowledgeBaseStore.fetchKnowledgeBases()
    }
    console.log('[Load] 获取知识库列表:', knowledgeBases)

    const paths = syncRepoPathsFromKnowledgeBases(knowledgeBases)

    if (!fileTreeStore.activeRepoPath && paths.length > 0) {
      console.log('[Set] 设置默认激活路径:', paths[0])
    }

    return paths
  } catch (error) {
    console.error('[Error] 初始化知识库失败:', error)
    throw error
  } finally {
    if (fileTreeStore.activeRepoPath) {
      await setupFileWatcher()
    }
  }
}

watch(
  () => knowledgeBaseStore.knowledgeBases,
  (list) => {
    syncRepoPathsFromKnowledgeBases(list)
  },
  { deep: true }
)

// 初始化时获取文件树数据
onMounted(async () => {
  // 1. 初始化知识库
  await initializeRepositories()

  // 2. 加载当前激活仓库的文件树
  if (fileTreeStore.activeRepoPath) {
    await loadRepoTree(fileTreeStore.activeRepoPath)
  }

  await nextTick()
  updateActiveRepoMaxHeight()

  window.addEventListener('resize', updateActiveRepoMaxHeight)

  if (typeof ResizeObserver !== 'undefined' && fileTreeContainerRef.value) {
    const ro = new ResizeObserver(() => updateActiveRepoMaxHeight())
    ro.observe(fileTreeContainerRef.value)

    const headerEl = fileTreeContainerRef.value.querySelector('.list-header')
    if (headerEl) ro.observe(headerEl)

    activeRepoResizeObserver.value = ro
  }

  // 添加文件监控
  // setupFileWatcher()
})


onUnmounted(() => {
  window.removeEventListener('resize', updateActiveRepoMaxHeight)
  activeRepoResizeObserver.value?.disconnect?.()
  activeRepoResizeObserver.value = null

  // 清理所有状态
  editingNode.value = null
  editName.value = ''
  selectedNode.value = null
  expandedKeys.value = []
  lastSelectedKey.value = null

  // 清理文件监控 - 不使用 await
  cleanupFileWatcher().catch(error => {
    console.error('清理文件监控时出错:', error)
  })
})

</script>

<style scoped>
/* 刷新图标样式 */
.refresh-icon {
  color: var(--n-text-color-secondary);
  cursor: pointer;
  transition: all 0.3s ease;
  padding: 4px;
}

.refresh-icon:hover {
  color: var(--n-color-primary);
  background-color: var(--n-color-primary-subtle);
  transform: rotate(90deg);
}

/* 暗色模式适配 */
.dark-mode .refresh-icon:hover {
  background-color: rgba(65, 105, 225, 0.2);
}

/* 新增的标题栏样式 */
.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 14px;
  margin-bottom: 0;
  background-color: var(--guaxi-header-bg);
  border-top-left-radius: 8px;
  border-top-right-radius: 8px;
  border-bottom: 1px solid var(--guaxi-border);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.title-container {
  display: flex;
  align-items: center;
  height: auto;
  /* 确保高度自适应 */
  gap: 6px;
}

.kb-manager-icon {
}

.repo-title-header {
  font-size: 15px !important;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  display: flex;
  align-items: center;
  margin: 0;
  padding: 0;
  line-height: 1.2;
}

.repo-title {
  font-size: 13px !important;
  font-weight: 500;
  color: var(--guaxi-text-primary);
  display: flex;
  align-items: center;
  margin: 0;
  padding: 0;
  line-height: 1.2;
}

/* 刷新按钮样式调整 */
.refresh-btn {
  background-color: transparent;
  color: var(--guaxi-text-secondary);
  border: none;
  transition: all 0.3s ease;
}

.refresh-btn:hover {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  transform: rotate(90deg);
}

.file-tree-container {
  height: calc(100% - var(--scrollbar-height, 42px));
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
  font-family: 'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.import-template-modal-content {
  width: 420px;
  max-width: 100%;
  box-sizing: border-box;
}

:deep(.import-template-modal-content .n-form),
:deep(.import-template-modal-content .n-form-item) {
  max-width: 100%;
}

:deep(.import-template-modal-content .n-form-item-blank) {
  flex: 1 1 0;
  min-width: 0;
}

:deep(.import-template-modal-content .n-input),
:deep(.import-template-modal-content .n-input-wrapper) {
  width: 100%;
  box-sizing: border-box;
}

:deep(.import-template-modal-content .n-input__input-el) {
  min-width: 0;
}

.toolbar {
  padding: 12px 16px;
  background-color: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title {
  font-size: 16px;
  font-weight: 600;
  color: var(--n-text-color);
}

.repo-body {
  flex: 1;
  height: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.active-repo-wrapper {
  background-color: rgba(0, 0, 0, 0.04);
  border-left: 3px solid #1890ff;
  box-shadow: inset 0 0 8px rgba(0, 0, 0, 0.08);
  border-bottom: 1px solid var(--n-border-color);
}

.active-repo-wrapper .repo-actions {
  opacity: 1;
}

.active-repo-wrapper .repo-title,
.active-repo-wrapper .repo-icon,
.active-repo-wrapper .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

.active-repo-wrapper .repo-item {
  padding: 12px 12px;
}

.active-tree-container {
  height: var(--active-repo-max-height, 360px);
  padding: 8px 0 8px 24px;
  border-left: 1px dashed var(--n-border-color);
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
}

.active-tree-container .tree-component {
  height: 100%;
}

.repo-scroll-container {
  flex: 1;
  height: 0;
  min-height: 0;
  background-color: transparent;
}

:deep(.repo-scroll-container .n-scrollbar-rail) {
  width: 6px;
}

:deep(.repo-scroll-container .n-scrollbar-rail__scrollbar) {
  border-radius: 3px;
}

.repo-list {
  border-radius: 0;
  background-color: transparent;
}

.repo-list-item {
  padding: 8px 12px;
  transition: all 0.2s ease;
  border-radius: 6px;
  border-bottom: 1px solid var(--n-border-color);
  position: relative;
  margin: 2px 0;
  overflow: hidden;
}

.repo-list-item:last-child {
  border-bottom: none;
}

/* 鼠标悬停效果 - 添加发光边框和背景 */
.repo-list-item:hover {
  background-color: var(--guaxi-header-bg);
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
}

/* 鼠标悬停时的文字和图标效果 */
.repo-list-item:hover .repo-title {
  color: #1890ff;
}

.repo-list-item:hover .repo-icon {
  color: #1890ff;
}

.repo-list-item:hover .expand-icon {
  color: #1890ff;
}

/* 选中状态效果*/
.repo-list-item.active {
  background-color: var(--guaxi-header-bg);
  border-left: 3px solid var(--n-color-primary, #1890ff);
  box-shadow: inset 0 0 8px rgba(0, 0, 0, 0.08);
  position: sticky;
  top: 0;
  z-index: 3;
}

.repo-list-item.active.expanded {
  height: var(--active-repo-max-height, 360px);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

:deep(.repo-list-item.active.expanded > .n-list-item__main) {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.repo-list-item.active.expanded .tree-container {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
}

.repo-list-item.active.expanded .tree-component {
  height: 100%;
}

:deep(.repo-list-item.active.expanded .tree-component .n-tree__empty) {
  height: 100%;
}

/* 暗色模式下选中状态更明显 */
.dark-mode .repo-list-item.active {
  background-color: rgba(255, 255, 255, 0.06);
  border-left: 3px solid #8ab4f8;
  box-shadow: inset 0 0 8px rgba(255, 255, 255, 0.08);
}

/* 选中状态下的文字和图标 */
.repo-list-item.active .repo-title,
.repo-list-item.active .repo-icon,
.repo-list-item.active .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

.repo-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  cursor: pointer;
  padding: 4px 0;
}

.repo-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: var(--n-text-color);
  transition: color 0.2s ease;
}

.repo-icon {
  color: var(--n-color-primary);
  margin-right: 8px;
  transition: color 0.2s ease;
}

.expand-icon {
  transition: all 0.2s ease;
  color: var(--n-text-color);
}

.expand-icon.rotated {
  transform: rotate(90deg);
}

.tree-container {
  position: relative;
  width: 100%;
  padding: 8px 0 8px 24px;
  border-left: 1px dashed var(--n-border-color);
}

.tree-component {
  width: 100%;
}

:deep(.tree-component .n-tree-node) {
  padding: 0;
}

:deep(.tree-component .n-tree-node-content) {
  margin: 2px 6px;
  padding: 4px 8px;
  border-radius: 6px;
  transition: transform 0.2s ease, box-shadow 0.2s ease, background 0.2s ease;
}

:deep(.tree-component .n-tree-node-content__text) {
  font-size: 13px;
  line-height: 1.35;
  font-family: 'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

:deep(.tree-component .n-tree-node-content:hover) {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.18), 0 2px 12px 0 rgba(7, 118, 161, 0.1);
  transform: scale(1.01);
}

:deep(.tree-component .n-tree-node--selected .n-tree-node-content) {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.26), 0 2px 12px 0 rgba(7, 118, 161, 0.14);
  animation: file-tree-select-pop 160ms ease-out;
}

@keyframes file-tree-select-pop {
  from {
    transform: scale(1);
  }
  60% {
    transform: scale(1.02);
  }
  to {
    transform: scale(1.01);
  }
}

/* 编辑输入框样式 */
.editable-input {
  border: 1px solid var(--n-border-color);
  border-radius: 4px;
  padding: 2px 4px;
  font-size: 14px;
  outline: none;
  width: 100%;
}

.editable-input:focus {
  border-color: var(--n-color-primary);
  box-shadow: 0 0 0 2px var(--n-color-primary-hover);
}

/* 刷新按钮动画 */
.refresh-btn {
  transition: transform 0.3s ease;
}

.refresh-btn:hover {
  transform: rotate(90deg);
}

/* 暗色模式适配 */
.dark-mode .file-tree-container {
  background-color: var(--n-card-color, #1a1a1a);
  border-color: rgba(255, 255, 255, 0.12);
}

.dark-mode .repo-scroll-container {
  background-color: transparent;
}

/* 选中状态下的 action/refresh 图标 */
.repo-list-item.active .action-icon,
.repo-list-item.active .refresh-icon,
.repo-list-item.active .browser-icon {
  color: var(--n-color-primary, #1890ff);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

/* 按下状态下的 action/refresh 图标 */
.action-icon:active,
.refresh-icon:active {
  color: var(--n-color-primary-hover, #1765ad);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

/* 暗色模式下 action/refresh 图标 */
.dark-mode .action-icon,
.dark-mode .refresh-icon,
.dark-mode .browser-icon,
.dark-mode .kb-manager-icon {
  color: #444a57;
  background-color: transparent;
}

.dark-mode .action-icon:hover,
.dark-mode .action-icon.active,
.dark-mode .refresh-icon:hover,
.dark-mode .refresh-icon.active,
.dark-mode .browser-icon:hover,
.dark-mode .browser-icon.active,
.dark-mode .kb-manager-icon:hover,
.dark-mode .kb-manager-icon.active,
.dark-mode .repo-list-item.active .action-icon,
.dark-mode .repo-list-item.active .refresh-icon,
.dark-mode .repo-list-item.active .browser-icon {
  color: #8ab4f8;
  background-color: rgba(65, 105, 225, 0.13);
}

/* 新增的样式 */
.repo-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.repo-list-item:hover .repo-actions {
  opacity: 1;
}

/* Action/refresh 图标基础样式（合并） */
.action-icon,
.refresh-icon,
.browser-icon,
.kb-manager-icon {
  color: var(--n-text-color-disabled, #bfbfbf);
  background-color: transparent;
  transition: color 0.2s, background-color 0.2s;
  border-radius: 4px;
  padding: 2px;
  cursor: pointer;
}

.action-icon:hover,
.action-icon.active,
.refresh-icon:hover,
.refresh-icon.active,
.browser-icon:hover,
.browser-icon.active,
.kb-manager-icon:hover,
.kb-manager-icon.active {
  color: var(--n-color-primary, #1890ff);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}
</style>