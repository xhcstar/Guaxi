<template>
  <div class="data-table-wrapper">
    <n-data-table ref="tableRef" :columns="columns" :data="data" :max-height="tableHeight" virtual-scroll
      virtual-scroll-x :scroll-x="scrollX" :min-row-height="40" :height-for-row="heightForRow" :header-height="50"
      :pagination="pagination" :single-line="false" class="data-table" :row-props="rowProps" :multiple="true"
      :row-key="rowKey" @update:checked-row-keys="$emit('update:checked-row-keys', $event)"
      @row-contextmenu="$emit('row-contextmenu', $event, $event.row)" striped flex-height />
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { NDataTable } from 'naive-ui'

defineProps({
  columns: {
    type: Array,
    required: true
  },
  data: {
    type: Array,
    required: true
  },
  tableHeight: {
    type: Number,
    default: 400
  },
  scrollX: {
    type: Number,
    default: 1200
  },
  pagination: {
    type: Object,
    default: () => ({ pageSize: 10 })
  },
  rowKey: {
    type: [String, Function],
    default: 'id'
  },
  rowProps: {
    type: Function,
    default: () => ({})
  },
  heightForRow: {
    type: Function,
    default: () => 85
  }
})

defineEmits(['update:checked-row-keys', 'row-contextmenu'])

const tableRef = ref(null)

defineExpose({
  tableRef,
  sort: (column, order) => tableRef.value?.sort(column, order),
  filter: (filters) => tableRef.value?.filter(filters),
  clearFilters: () => tableRef.value?.filter(null),
  clearSorter: () => tableRef.value?.sort(null)
})
</script>

<style scoped>
.data-table-wrapper {
  flex: 1;
  width: 100%;
  overflow: hidden;
  padding: 8px 8px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  /* 确保父容器有明确的高度 */
}

.data-table {
  flex: 1;
  width: 100%;
  min-height: 0;
  height: 100%;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

/* 更精确的样式选择器 */
:deep(.n-data-table-base) {
  height: 100% !important;
  display: flex;
  flex-direction: column;
}

:deep(.n-data-table-wrapper) {
  flex: 1;
  min-height: 0;
}

:deep(.n-data-table-table) {
  flex: 1;
  min-height: 0;
}

:deep(.n-data-table) {
  --n-border-color: #eaeaea;
  --n-th-color: #f8f9fa;
  --n-td-color: #ffffff;
  --n-td-color-hover: #f5f7fa;
  --n-td-color-striped: #f9f9f9;
  height: 100% !important;
  display: flex;
  flex-direction: column;
}

:deep(.n-data-table-th) {
  font-weight: 600;
  color: #333;
  padding: 16px;
}

:deep(.n-data-table-td) {
  padding: 12px 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:deep(.n-data-table-tr--hover) {
  background-color: #f0f7ff;
}
</style>