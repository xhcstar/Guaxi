import { defineStore } from "pinia";

export const repoStore = defineStore("repoStore", {
  state: () => ({
    files: [], // 存储文件数据
    notes: [], // 存储笔记数据
    currentDatabase: "importFileDB", // 默认选中的数据库
    librariesRoot: "",
  }),
  actions: {
    // 根据 key 获取文件或笔记数据
    async fetchFiles(key) {
      // 模拟不同数据库的数据
      const data = {
        importFileDB: [
          {
            id: 1,
            importFileName: "文件1.pdf",
            description: "这是文件1",
            tags: "标签1,标签2",
            type: "pdf",
            library: "Library1",
            importFileCreateTime: "2025-03-06 08:00:00",
            importFileSize: 2048,
            convetToHtmlFilename: "file1.html",
            convetToMarkdownFilename: "file1.md",
          },
        ],
        NotesDb: [
          {
            id: 1,
            description: "这是笔记1",
            noteFileName: "file1.md",
            tags: "笔记,学习",
            fromImportedFile: "file1.md",
            fromImportedFileLibrary: "Library1",
            noteFileType: "markdown",
            noteFileCreateTime: "2025-03-06 08:00:00",
            noteFileUpdateTime: "2025-03-06 09:00:00",
            parent_note: null,
            child_notes: [],
          },
          {
            id: 2,
            description: "这是笔记2",
            noteFileName: "file2.md",
            tags: "笔记,工作",
            fromImportedFile: "file1.md",
            fromImportedFileLibrary: "Library2",
            noteFileType: "markdown",
            noteFileCreateTime: "2025-03-06 10:00:00",
            noteFileUpdateTime: "2025-03-06 11:00:00",
            parent_note: 1,
            child_notes: [3],
          },
          {
            id: 3,
            description: "这是笔记3",
            noteFileName: "file3.md",
            tags: "笔记,工作",
            fromImportedFile: "file3.md",
            fromImportedFileLibrary: "Library3",
            noteFileType: "markdown",
            noteFileCreateTime: "2025-03-06 10:00:00",
            noteFileUpdateTime: "2025-03-06 11:00:00",
            parent_note: 1,
            child_notes: [3],
          },
        ],
      };

      if (this.currentDatabase === "NotesDb") {
        this.notes = data[this.currentDatabase] || []; // 加载笔记数据
      } else {
        this.files = data[this.currentDatabase] || []; // 加载文件数据
      }
    },

    // 添加导入的文件
    addImportedFile(file) {
      this.files.push(file);
    },

    // 添加导入的笔记
    addImportedNote(note) {
      this.notes.push(note);
    },

    set_librariesRoot(path) {
      this.librariesRoot = path;
    },
  },
});