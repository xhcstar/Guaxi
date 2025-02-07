<template>
  <n-dropdown :options="dropdownOptions" placement="bottom-start" trigger="click" @select="handleModelChange">
    <n-tag round :bordered="false" size="small">
      切换模型
      <template #icon>
        <n-icon size="16" style="cursor: pointer;">
          <svg t="1736674014610" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
            p-id="13379" width="16" height="16">
            <path
              d="M509.976 715.584c15.872 0 30.08-7.12 39.624-18.328a46.64 46.64 0 0 0 10.4-7.912L854.24 392.4a47.296 47.296 0 0 0-0.08-66.664l-0.224-0.216a46.664 46.664 0 0 0-65.992 0.296L509.976 606.344 232 325.816l-1.456-1.408a46.664 46.664 0 0 0-64.544 1.112l-0.216 0.216-1.416 1.48a47.296 47.296 0 0 0 1.328 65.184l294.256 296.944 1.52 1.472c2.752 2.536 5.736 4.68 8.88 6.44a51.888 51.888 0 0 0 39.624 18.32z"
              fill="#666666" p-id="13380"></path>
          </svg>
        </n-icon>
      </template>
    </n-tag>
  </n-dropdown>
</template>

<script lang="ts">
import type { DropdownOption } from 'naive-ui'
import { defineComponent, computed } from 'vue'
import { NIcon } from 'naive-ui'
import { useChatStore } from "../../stores/chatStore";


export default defineComponent({
  setup() {
    // 引入 Pinia 的 ChatStore
    const chatStore = useChatStore();

    // 动态生成 options
    const dropdownOptions = computed<DropdownOption[]>(() => {
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
    const handleModelChange = (key: string) => {
      console.log('key', key);
      const selectModelId = parseInt(key.split('-')[1], 10);
      console.log('selectModelId', selectModelId);
      chatStore.setChatModel(selectModelId);
    };

    return {
      dropdownOptions,
      handleModelChange,
    };
  }
})
</script>