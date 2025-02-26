import { defineStore } from "pinia";

export const useFileProcessStore = defineStore("fileProcessStore", {
  state: () => ({
    supportFileType: [
      { id: "0", type: "txt" },
      { id: "1", type: "pdf" },
      { id: "2", type: "docx" },
      { id: "3", type: "md" }
    ],
    fileTypeIcon: [],
    fileContent: {},
    selectedOpenFile: [],
  }),
  actions: {},
});
