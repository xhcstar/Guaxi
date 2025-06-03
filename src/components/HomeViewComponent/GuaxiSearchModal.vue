<template>
  <n-modal v-model:show="show" :mask-closable="false">
    <n-card style="width: 800px; height: 600px">
      <n-tabs type="line">
        <n-tab-pane name="files" tab="文件">
          <n-list>
            <n-list-item v-for="item in results" :key="item.path">
              <n-thing :title="item.fileName" :description="item.path" @click="openFile(item.path)"/>
            </n-list-item>
          </n-list>
        </n-tab-pane>
        <n-tab-pane name="notes" tab="笔记">
          <n-list>
            <n-list-item v-for="item in results" :key="item.path">
              <n-thing :title="item.path" :description="item.preview" @click="openFile(item.path)"/>
            </n-list-item>
          </n-list>
        </n-tab-pane>
      </n-tabs>
    </n-card>
  </n-modal>
</template>

<script setup>
import { ref } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'

const props = defineProps({
  show: Boolean,
  results: Array
})

const emit = defineEmits(['update:show'])

const openFile = (path) => {
  emit('update:show', false)
  // 触发打开文件逻辑
}
</script>