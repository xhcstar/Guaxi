<template>
  <div class="data-table-wrapper">
    <n-data-table ref="tableRef" :columns="columns" :data="data" :max-height="tableHeight" virtual-scroll
      :scroll-x="scrollX" :min-row-height="70" :height-for-row="heightForRow" :header-height="50"
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

const emit = defineEmits(['update:checked-row-keys', 'row-contextmenu'])

const tableRef = ref(null)

defineExpose({
  tableRef,
  sort: (column, order) => tableRef.value?.sort(column, order),
  filter: (filters) => tableRef.value?.filter(filters),
  clearFilters: () => tableRef.value?.filter(null),
  clearSorter: () => tableRef.value?.sort(null),
  // naive n-data-table 没有 clearSelection:选中态由父组件通过 checked-row-keys 控制,
  // 这里发一个空数组让父组件清空选择(修复 resetTableState 调用不存在方法的报错)。
  clearSelection: () => emit('update:checked-row-keys', [])
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
}

.data-table {
  flex: 1;
  width: 100%;
  min-height: 0;
  height: 100%;
  border-radius: 8px;
  overflow: hidden;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

/* 移除或修改单元格padding设置 */
:deep(.n-data-table-td) {
  padding: 2px 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  position: relative;
}

/* 确保单元格内容完全填充 */
:deep(.n-data-table-td .n-data-table-td__cell) {
  height: 100%;
  display: flex;
  align-items: center;
  box-sizing: border-box;
}

:deep(.n-data-table-td .n-data-table-td__cell .editable-cell.is-edit) {
  height: 100%;
  align-self: stretch;
}

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
  --n-border-color: var(--guaxi-border);
  --n-th-color: var(--guaxi-header-bg);
  --n-td-color: var(--guaxi-panel-bg);
  --n-td-color-hover: var(--guaxi-header-bg);
  --n-td-color-striped: var(--guaxi-content-bg);
  height: 100% !important;
  display: flex;
  flex-direction: column;
}

:deep(.n-data-table .n-data-table__pagination) {
  background-color: var(--guaxi-panel-bg);
  border-top: 1px solid var(--guaxi-border);
}

:deep(.n-data-table .n-pagination-quick-jumper) {
  color: var(--guaxi-text-secondary);
}

:deep(.n-data-table .n-pagination-quick-jumper .n-input .n-input-wrapper) {
  background-color: var(--guaxi-panel-bg) !important;
}

:deep(.n-data-table .n-pagination-quick-jumper .n-input .n-input__border),
:deep(.n-data-table .n-pagination-quick-jumper .n-input .n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

:deep(.n-data-table .n-pagination-quick-jumper .n-input__input-el) {
  color: var(--guaxi-text-primary) !important;
}

:deep(.n-data-table .n-pagination-quick-jumper .n-input__input-el::placeholder) {
  color: var(--guaxi-text-secondary) !important;
}

:deep(.n-data-table-th) {
  font-weight: 600;
  color: var(--guaxi-text-primary);
  padding: 10px;
}

:deep(.n-data-table-tr--hover) {
  background-color: var(--guaxi-header-bg);
}
</style>