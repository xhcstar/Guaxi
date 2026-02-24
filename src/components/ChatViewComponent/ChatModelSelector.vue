<template>
  <n-dropdown :options="dropdownOptions" placement="bottom-start" trigger="click" @select="handleModelChange">
    <n-tag round :bordered="false" size="small">
      切换模型
      <template #icon>
        <n-icon size="16" :component="ChevronDown16Filled" style="cursor: pointer;"></n-icon>
      </template>
    </n-tag>
  </n-dropdown>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed } from 'vue'
import { useChatStore } from "../../stores/chatStore";
import { ChevronDown16Filled } from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('ChatModelSelector')

// 引入 Pinia 的 ChatStore
const chatStore = useChatStore();

// 动态生成 options
const dropdownOptions = computed(() => {
  const { LLMlist } = chatStore;

  return [
    {
      type: "group",
      label: "远程模型",
      key: "remote",
      children: LLMlist.remoteModel.map((model) => ({
        label: model.name,
        key: `remote-${model.selectedModel}`,
      })),
    },
    {
      type: "group",
      label: "本地模型",
      key: "local",
      children: LLMlist.localModel.map((model) => ({
        label: model.name,
        key: `local-${model.selectedModel}`,
      })),
    },
  ];
});

// 处理模型选择变化
const handleModelChange = async (key) => {
  logger.info('key', key);
  const selectModelId = parseInt(key.split('-')[1], 10);
  logger.info('selectModelId', selectModelId);
  chatStore.setChatModel(selectModelId);

  if (selectModelId === 6) {
    try {
      await invoke('ensure_local_deepseek_service')
    } catch (e) {
      logger.error('LocalDeepseek 预启动失败:', e)
    }
  }
};
</script>
