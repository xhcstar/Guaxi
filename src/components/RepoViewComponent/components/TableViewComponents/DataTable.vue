<template>
  <n-data-table ref="tableRef" :columns="columns" :data="data" :max-height="tableHeight" virtual-scroll virtual-scroll-x
    :scroll-x="scrollX" :min-row-height="85" :height-for-row="heightForRow" :header-height="85" :pagination="pagination"
    :single-line="false" class="data-table" :row-props="rowProps" :multiple="true" :row-key="rowKey"
    @update:checked-row-keys="$emit('update:checked-row-keys', $event)"
    @row-contextmenu="$emit('row-contextmenu', $event, $event.row)" striped />
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
    default: () => ({ pageSize: 15 })
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

// 暴露方法给父组件
defineExpose({
  tableRef,
  sort: (column, order) => tableRef.value?.sort(column, order),
  filter: (filters) => tableRef.value?.filter(filters),
  clearFilters: () => tableRef.value?.filter(null),
  clearSorter: () => tableRef.value?.sort(null)
})
</script>

<style scoped>
.data-table {
  flex: 1;
  width: 100% !important;
  height: 100%;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

:deep(.n-data-table) {
  --n-border-color: #eaeaea;
  --n-th-color: #f8f9fa;
  --n-td-color: #ffffff;
  --n-td-color-hover: #f5f7fa;
  --n-td-color-striped: #f9f9f9;
}

:deep(.n-data-table-th) {
  font-weight: 600;
  color: #333;
  padding: 16px;
}

:deep(.n-data-table-td) {
  padding: 12px 16px;
}

:deep(.n-data-table-tr--hover) {
  background-color: #f0f7ff;
}
</style>