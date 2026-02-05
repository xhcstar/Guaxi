import { defineStore } from "pinia";
import { createLogger } from "../utils/logger";

const logger = createLogger("fileProcessStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

export const useFileProcessStore = defineStore("fileProcessStore", {
  state: () => ({
    supportFileType: [
      { id: "0", type: "txt" },
      { id: "1", type: "pdf" },
      { id: "2", type: "docx" },
      { id: "3", type: "doc" },
      { id: "4", type: "md" },
      { id: "5", type: "html" },
      { id: "6", type: "epub" },
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
      console.log("SSS>>> getCurentOpenedFileContent response", response);

      // 查找是否已经存在对应的页面
      let pageFound = false;
      this.openedCurentfileContent.forEach((page, index) => {
        if (page.active === active) {
          // 更新已存在的页面
          this.openedCurentfileContent[index] = {
            active,
            fileName: this.selectedFile,
            fileType: response.filetype,
            content: response.content,
            totalPages: response.totalpages,
          };
          pageFound = true;
          console.log("pageFound:", this.openedCurentfileContent[index]);
        }
      });

      if (!pageFound) {
        // 如果未找到对应的页面，添加新页面
        this.openedCurentfileContent.push({
          active,
          fileName: this.selectedFile,
          fileType: response.filetype,
          content: response.content,
          totalPages: response.totalpages,
        });
        console.log("pageFound:", this.openedCurentfileContent);
      }

      console.log(
        "openedCurentfileContent after update:",
        this.openedCurentfileContent
      );
    },
  },
});
