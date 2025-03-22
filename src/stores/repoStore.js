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
            title: "文件1.pdf",
            type: "pdf",
            create_time: "2025-03-06 08:00:00",
            size: 2048,
            path: "/path/to/file1.pdf",
            library: "Library1",
            description: "这是文件1",
            tags: "标签1,标签2",
            html_file_name: "file1.html",
            markdown_file_name: "file1.md",
          },
        ],
        NotesDb: [
          {
            id: 1,
            title: "笔记1",
            description: "这是笔记1",
            type: "markdown",
            tags: "笔记,学习",
            create_time: "2025-03-06 08:00:00",
            update_time: "2025-03-06 09:00:00",
            library: "Library1",
            file: "file1.md",
            parent_note: null,
            child_notes: [],
          },
          {
            id: 2,
            title: "笔记2",
            description: "这是笔记2",
            type: "markdown",
            tags: "笔记,工作",
            create_time: "2025-03-06 10:00:00",
            update_time: "2025-03-06 11:00:00",
            library: "Library2",
            file: "file2.md",
            parent_note: 1,
            child_notes: [3],
          },
          {
            id: 3,
            title: "笔记3",
            description: "这是笔记3",
            type: "markdown",
            tags: "笔记,生活",
            create_time: "2025-03-06 12:00:00",
            update_time: "2025-03-06 13:00:00",
            library: "Library2",
            file: "file2.md",
            parent_note: 2,
            child_notes: [],
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