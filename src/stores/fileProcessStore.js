import { defineStore } from "pinia";

export const useFileProcessStore = defineStore("fileProcessStore", {
  state: () => ({
    supportFileType: [
      { id: "0", type: "txt" },
      { id: "1", type: "pdf" },
      { id: "2", type: "docx" },
      { id: "3", type: "md" },
    ],
    fileTypeIcon: [],
    openedCurentfileContent: [],
    selectedFile: "",
    curentPage: 1,
    lastPage: 0,
  }),
  actions: {
    setSelectedFile(file) {
      this.selectedFile = file;
      this.curentPage = 1;
      this.lastPage = 0;
      console.log("selectedFile:", this.selectedFile);
    },
    getCurentOpenedFileContent(response, active) {
      this.openedCurentfileContent.push({
        active: active,
        fileName: this.selectedFile,
        fileType: response.filetype,
        content: response.content,
      });
      console.log("openedCurentfileContent:", this.openedCurentfileContent);
    },
  },
});
