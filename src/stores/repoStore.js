import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export const repoStore = defineStore("repoStore", {
  state: () => ({
    files: [], // 存储文件数据
    notes: [], // 存储笔记数据
    currentDatabase: "importFileDB", // 默认选中的数据库
    librariesRoot: "",
  }),
  actions: {
    async fetchFiles(key) {
      try {
        if (key === "NotesDb") {
          // 调用获取笔记数据的接口
          const notes = await invoke("get_notes");
          console.log("get_notes", notes);

          // 数据转换：将后端笔记数据映射为前端需要的数据结构
          this.notes = notes.map((note) => ({
            id: note.id,
            description: note.description,
            noteFileName: note.note_file_name,
            noteFileType: note.note_file_type,
            tags: note.tags.join(","), // 将 tags 数组转换为字符串
            fromImportedFile: note.from_imported_file,
            fromImportedFileLibrary: note.from_imported_file_library,
            noteFileCreateTime: note.note_file_create_time,
            noteFileUpdateTime: note.note_file_update_time,
          }));
        } else {
          // 调用获取文件数据的接口
          const files = await invoke("get_imported_files");
          console.log("get_imported_files", files);

          // 数据转换：将后端文件数据映射为前端需要的数据结构
          this.files = files.map((file) => ({
            id: file.id,
            importFileName: file.import_file_name,
            description: file.description,
            tags: file.tags.join(","), // 将 tags 数组转换为字符串
            type: file.file_type,
            library: file.library,
            importFileCreateTime: file.import_file_create_time,
            importFileSize: file.import_file_size,
            convetToHtmlFilename: `${file.import_file_name}.html`,
            convetToMarkdownFilename: `${file.import_file_name}.md`,
          }));
        }
      } catch (error) {
        console.error("Error fetching data:", error);
        this.notes = []; // 重置笔记数据
        this.files = []; // 重置文件数据
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
      console.log("set_librariesRoot", this.librariesRoot);
    },

    async get_librariesRoot() {
      try {
        this.librariesRoot = await invoke('get_libraries_root')
        console.log('初始化：获取当librariesRoot路径成功', this.librariesRoot)
      } catch (error) {
        console.error('获取当librariesRoot仓库路径失败', error)
      }
    },
  },
});
