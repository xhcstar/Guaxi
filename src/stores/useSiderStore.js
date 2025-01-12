import { defineStore } from 'pinia'

export const useSiderStore = defineStore('sider', {
  state: () => ({
    isLeftSiderCollapsed: false,
    isRightSiderCollapsed: true,
  }),
  actions: {
    toggleLeftSider() {
      this.isLeftSiderCollapsed = !this.isLeftSiderCollapsed
    },
    toggleRightSider() {
      this.isRightSiderCollapsed = !this.isRightSiderCollapsed
    },
  },
})
