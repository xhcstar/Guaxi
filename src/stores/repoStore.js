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
        // 调用获取文件数据的接口
        const files = await invoke("get_imported_files", { knowledgeRepo: key });
        console.log("get_imported_files", files);

        // 数据转换：将后端文件数据映射为前端需要的数据结构
        this.files = files.map((file) => ({
          id: file.id,
          importFileName: file.import_file_name,
          description: file.description,
          importedFileTags: file.tags.join(","), // 将 tags 数组转换为字符串
          type: file.file_type,
          library: file.library,
          importFileCreateTime: file.import_file_create_time,
          importFileSize: file.import_file_size,
          convetToHtmlFilename: "",
          convetToMarkdownFilename: file.converted_markdown_filename,
          knowledgeRepo: file.knowledge_repo,
          newNoteForm: {
            id: file.note_file.id || 0, // 如果没有 id，则默认为 0,
            description: file.note_file.description,
            noteFileName: file.note_file.note_file_name,
            noteFileType: file.note_file.note_file_type,
            notesTags: file.note_file.tags.join(","), // 将 tags 数组转换为字符串
            fromImportedFile: file.note_file.from_imported_file,
            fromImportedFileLibrary: file.note_file.from_imported_file_library,
            noteFileCreateTime: file.note_file.note_file_create_time,
            noteFileUpdateTime: file.note_file.note_file_update_time,
            knowledgeRepo: file.note_file.knowledge_repo,
          },
        }));
        console.log("文件数据获取成功", this.files);
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
        this.librariesRoot = await invoke("get_libraries_root");
        console.log("初始化：获取当librariesRoot路径成功", this.librariesRoot);
      } catch (error) {
        console.error("获取当librariesRoot仓库路径失败", error);
      }
    },
  },
});
