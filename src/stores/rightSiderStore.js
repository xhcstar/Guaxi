import { defineStore } from 'pinia'

export const useRightSiderStore = defineStore('rightSider', {
  state: () => ({
    searchResults: {
      fileResults: [],
      noteResults: [],
      searchQuery: ''
    }
  }),
  actions: {
    setSearchResults(results) {
      this.searchResults = results
    },
    clearSearchResults() {
      this.searchResults = {
        fileResults: [],
        noteResults: [],
        searchQuery: ''
      }
    }
  }
})