"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
const repoStore_1 = require("../../stores/repoStore");
const fluent_1 = require("@vicons/fluent");
const naive_ui_1 = require("naive-ui");
const ShowOrEdit_vue_1 = __importDefault(require("./ShowOrEdit.vue"));
const useSearch_1 = require("./SearchUtils/useSearch");
const plugin_dialog_1 = require("@tauri-apps/plugin-dialog");
const core_1 = require("@tauri-apps/api/core");
const plugin_os_1 = require("@tauri-apps/plugin-os");
const event_1 = require("@tauri-apps/api/event");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
// import { Window } from '@tauri-apps/api/window';
// import { Webview } from "@tauri-apps/api/webview"
// 引入 Pinia store
const store = (0, repoStore_1.repoStore)();
// 获取 message 实例
const message = (0, naive_ui_1.useMessage)();
// 添加文件对话框状态
const isAddFileModalVisible = (0, vue_1.ref)(false);
// 表格实例
const tableRef = (0, vue_1.ref)(null);
// 分页配置
const pagination = (0, vue_1.ref)({ pageSize: 15 });
// 用于 importFileDB 库的表单
const newFileForm = (0, vue_1.ref)({
    id: 0,
    filePath: '',
    importFileName: '',
    description: '',
    importedFileTags: '',
    type: '',
    library: '', // 所属库名字
    importFileCreateTime: '',
    importFileSize: 0,
    convetToHtmlFilename: '',
    importFileSize: '',
});
// 用于 NotesDb 库的表单
const newNoteForm = (0, vue_1.ref)({
    id: 0,
    filePath: '',
    description: '',
    noteFileName: '',
    notesTags: '', // 所属库名字
    fromImportedFile: '',
    fromImportedFileLibrary: '',
    noteFileType: '',
    noteFileCreateTime: '',
    noteFileUpdateTime: '', // 笔记特有的字段
});
// 当前表单（根据数据库类型动态切换）
const currentForm = (0, vue_1.computed)(() => {
    return store.currentDatabase === 'importFileDB' ? newFileForm.value : newNoteForm.value;
});
// 进度条相关状态
const progresses = (0, vue_1.ref)([]);
const startProgressListener = (filePath) => __awaiter(void 0, void 0, void 0, function* () {
    const fileName = filePath.split(/[\\/]/).pop() || '未知文件';
    const newProgress = {
        show: true,
        current: 0,
        total: 100,
        message: '开始转换...',
        fileName,
    };
    progresses.value.push(newProgress);
    const progressIndex = progresses.value.length - 1;
    const unlisten = yield (0, event_1.listen)('conversion_progress', (event) => {
        // 使用Vue的响应式更新方式
        progresses.value[progressIndex] = Object.assign(Object.assign({}, progresses.value[progressIndex]), { current: event.payload.current, message: event.payload.message });
        console.log('conversion_progress', event.payload.current, event.payload.message);
        // 转换完成后移除进度条
        if (event.payload.current >= 100) {
            setTimeout(() => {
                progresses.value = progresses.value.filter((_, i) => i !== progressIndex);
            }, 1000);
        }
    });
    return unlisten;
});
const convertFile = (inputPath, outputDir) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        yield (0, core_1.invoke)('convert_to_markdown', {
            request: {
                input_path: inputPath,
                output_dir: outputDir
            }
        }, {
            target: 'main'
        });
    }
    catch (error) {
        console.error('Conversion failed:', error);
        throw error;
    }
});
// 自定义校验函数：检查文件路径是否合法（根据系统类型动态选择匹配方式）
const validateFilePath = (rule, value) => __awaiter(void 0, void 0, void 0, function* () {
    console.log('validateFilePath', value); // 添加日志
    // 检查是否为空
    if (!value || value.trim() === '') {
        return new Error('文件路径不能为空');
    }
    // 获取当前操作系统类型
    const osPlatform = yield (0, plugin_os_1.platform)();
    // Windows 路径正则表达式
    const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
    // Linux 路径正则表达式
    const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
    // macOS 路径正则表达式（与 Linux 相同）
    const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
    console.log('osPlatform', osPlatform);
    // 根据系统类型选择匹配方式
    if (osPlatform === 'windows') {
        if (!windowsPathRegex.test(value)) {
            return new Error('Windows 文件路径不合法，请检查路径格式');
        }
    }
    else if (osPlatform === 'linux') {
        if (!linuxPathRegex.test(value)) {
            return new Error('Linux 文件路径不合法，请检查路径格式');
        }
    }
    else if (osPlatform === 'macos') {
        if (!macPathRegex.test(value)) {
            return new Error('macOS 文件路径不合法，请检查路径格式');
        }
    }
    else {
        return new Error('不支持的操作系统');
    }
    return true; // 校验通过
});
// 自定义校验函数：检查标签是否合法（单个字符串或以逗号分隔的字符串）
const validateTags = (rule, value) => {
    console.log('validateTags', value); // 添加日志
    // 检查是否为空
    if (!value || value.trim() === '') {
        return new Error('标签不能为空');
    }
    // 检查是否为单个字符串或以逗号分隔的字符串
    const tags = value.split(',').map(tag => tag.trim());
    if (tags.some(tag => !tag)) {
        return new Error('标签不能为空或以多个逗号分隔');
    }
    return true; // 校验通过
};
// 校验表单
const validateForm = () => __awaiter(void 0, void 0, void 0, function* () {
    const form = currentForm.value;
    console.log('>>>>>>>>>>>> validateForm', currentForm.value);
    // 校验文件路径
    if (!form.filePath || form.filePath.trim() === '') {
        return '文件路径不能为空';
    }
    // 获取当前操作系统类型
    const osPlatform = yield (0, plugin_os_1.platform)();
    console.log('osPlatform', osPlatform);
    // Windows 路径正则表达式
    const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
    // Linux 路径正则表达式
    const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
    // macOS 路径正则表达式（与 Linux 相同）
    const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
    // 根据系统类型选择匹配方式
    if (osPlatform === 'windows') {
        if (!windowsPathRegex.test(form.filePath)) {
            return 'Windows 文件路径不合法，请检查路径格式';
        }
    }
    else if (osPlatform === 'linux') {
        if (!linuxPathRegex.test(form.filePath)) {
            return 'Linux 文件路径不合法，请检查路径格式';
        }
    }
    else if (osPlatform === 'macos') {
        if (!macPathRegex.test(form.filePath)) {
            return 'macOS 文件路径不合法，请检查路径格式';
        }
    }
    else {
        return '不支持的操作系统';
    }
    // 校验标签
    if (store.currentDatabase === 'importFileDB') {
        if (!form.importedFileTags || form.importedFileTags.trim() === '') {
            console.log('>>>>>>>>>>>>> form', form.importedFileTags);
            console.log('>>>>>>>>>>>>> form.importedFileTags', form.importedFileTags);
            currentForm.value.importedFileTags = "default"; // 设置默认值
        }
        else {
            const tags = form.importedFileTags.split(',').map(tag => tag.trim());
            if (tags.some(tag => !tag)) {
                return '多个标签没有以逗号分隔';
            }
        }
    }
    else if (store.currentDatabase === 'NotesDb') {
        if (!form.notesTags || form.notesTags.trim() === '') {
            console.log('form.notesTags', form.notesTags);
            currentForm.value.notesTags = "default"; // 设置默认值
        }
        else {
            const tags = form.notesTags.split(',').map(tag => tag.trim());
            if (tags.some(tag => !tag)) {
                return '多个标签没有以逗号分隔';
            }
        }
    }
    else {
        return '不支持的数据库类型';
    }
    // 校验描述
    if (!form.description || form.description.trim() === '') {
        return '描述不能为空';
    }
    if (form.description.length > 240) {
        return '描述不能超过 240 个字符';
    }
    // 校验所属库
    if (store.currentDatabase === 'importFileDB') {
        if (!form.library || (typeof form.library === 'string' && form.library.trim() === '')) {
            currentForm.value.library = 'default'; // 设置默认值
            return '所属库不能为空';
        }
    }
    // 校验入库时间
    if (store.currentDatabase === 'importFileDB') {
        if (!form.importFileCreateTime || form.importFileCreateTime.trim() === '') {
            return '入库时间不能为空';
        }
    }
    // 校验文件大小
    if (store.currentDatabase === 'importFileDB') {
        if (!form.importFileSize || form.importFileSize <= 0) {
            return '文件大小不能为空';
        }
    }
    // 校验笔记文件名（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!form.noteFileName || form.noteFileName.trim() === '')) {
        return '笔记文件名不能为空';
    }
    // 校验笔记创建时间（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!form.noteFileCreateTime || form.noteFileCreateTime.trim() === '')) {
        return '笔记创建时间不能为空';
    }
    // 校验笔记的导入文件所属库（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!form.fromImportedFileLibrary || form.fromImportedFileLibrary.trim() === '')) {
        currentForm.value.fromImportedFileLibrary = "NoImportFileAssocNoteLib"; // 设置默认值
    }
    // 校验笔记的导入文件（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!form.fromImportedFile || form.fromImportedFile.trim() === '')) {
        currentForm.value.fromImportedFile = "NoImportFileAssocNote"; // 设置默认值
    }
    // 校验通过
    return null;
});
// 校验table cell
const validateTableCell = (row) => {
    console.log('validateTableCell', row);
    console.log('store.currentDatabase', store.currentDatabase);
    // 校验标签
    if (store.currentDatabase === 'importFileDB') {
        if (!row.importedFileTags || row.importedFileTags.trim() === '') {
            console.log('row.importedFileTags', row.importedFileTags);
            row.importedFileTags = "default"; // 设置默认值
        }
        else {
            const tags = row.importedFileTags.split(',').map(tag => tag.trim());
            if (tags.some(tag => !tag)) {
                return '多个标签没有以逗号分隔';
            }
        }
    }
    else if (store.currentDatabase === 'NotesDb') {
        if (!row.notesTags || row.notesTags.trim() === '') {
            console.log('row.notesTags', row.notesTags);
            row.notesTags = "default"; // 设置默认值
        }
        else {
            const tags = row.notesTags.split(',').map(tag => tag.trim());
            if (tags.some(tag => !tag)) {
                return '多个标签没有以逗号分隔';
            }
        }
    }
    else {
        return '不支持的数据库类型';
    }
    // 校验描述
    if (!row.description || row.description.trim() === '') {
        return '描述不能为空';
    }
    if (row.description.length > 240) {
        return '描述不能超过 240 个字符';
    }
    // 校验所属库
    if (store.currentDatabase === 'importFileDB') {
        if (!row.library || (typeof row.library === 'string' && row.library.trim() === '')) {
            row.library = 'default'; // 设置默认值
            return '所属库不能为空';
        }
    }
    // 校验入库时间
    if (store.currentDatabase === 'importFileDB') {
        if (!row.importFileCreateTime || row.importFileCreateTime.trim() === '') {
            return '入库时间不能为空';
        }
    }
    // 校验文件大小
    if (store.currentDatabase === 'importFileDB') {
        if (!row.importFileSize || row.importFileSize <= 0) {
            return '文件大小不能为空';
        }
    }
    // 校验笔记文件名（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!row.noteFileName || row.noteFileName.trim() === '')) {
        return '笔记文件名不能为空';
    }
    // 校验笔记创建时间（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!row.noteFileCreateTime || row.noteFileCreateTime.trim() === '')) {
        return '笔记创建时间不能为空';
    }
    // 校验笔记的导入文件所属库（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!row.fromImportedFileLibrary || row.fromImportedFileLibrary.trim() === '')) {
        row.fromImportedFileLibrary = "NoImportFileAssocNoteLib"; // 设置默认值
    }
    // 校验笔记的导入文件（如果是 NotesDb）
    if (store.currentDatabase === 'NotesDb' && (!row.fromImportedFile || row.fromImportedFile.trim() === '')) {
        row.fromImportedFile = "NoImportFileAssocNote"; // 设置默认值
    }
    // 校验通过
    return null;
};
// 表单校验规则
const rules = {
    filePath: [
        { validator: validateFilePath, required: true, message: '文件路径不能为空', trigger: ['input', 'blur'] },
    ],
    notesTags: [
        { validator: validateTags, required: true, message: '标签不能为空', trigger: ['input', 'blur'] },
    ],
    importedFileTags: [
        { validator: validateTags, required: true, message: '标签不能为空', trigger: ['input', 'blur'] },
    ],
    description: [
        { required: true, message: '描述不能为空', trigger: ['input', 'blur'] },
        { max: 240, message: '描述不能超过240个字符', trigger: ['input', 'blur'] },
    ],
    library: [
        { required: true, message: '所属库不能为空', trigger: ['input', 'blur'] },
    ],
    importFileCreateTime: [
        { required: true, message: '入库时间不能为空', trigger: ['input', 'blur'] },
    ],
    importFileSize: [
        { required: true, message: '文件大小不能为空', trigger: ['input', 'blur'] },
    ],
    noteFileName: [
        { required: true, message: '笔记文件名不能为空', trigger: ['input', 'blur'] },
    ],
    noteFileCreateTime: [
        { required: true, message: '笔记创建时间不能为空', trigger: ['input', 'blur'] },
    ],
};
// 动态生成表单列配置
const formColumns = (0, vue_1.computed)(() => {
    const columns = store.currentDatabase === 'importFileDB'
        ? [
            { key: 'filePath', title: '文件路径' },
            { key: 'importFileName', title: '文件名' },
            { key: 'description', title: '文件描述' },
            { key: 'library', title: '所属库' },
            { key: 'importedFileTags', title: '文件标签' },
            { key: 'type', title: '文件类型' },
            { key: 'importFileCreateTime', title: '入库时间' },
            { key: 'importFileSize', title: '文件大小' },
        ]
        : [
            { key: 'filePath', title: '文件路径' },
            { key: 'description', title: '笔记描述' },
            { key: 'noteFileName', title: '笔记文件名' },
            { key: 'notesTags', title: '笔记标签' },
            { key: 'noteFileType', title: '笔记文件类型' },
            { key: 'fromImportedFile', title: '笔记归属导入文件' },
            { key: 'fromImportedFileLibrary', title: '导入文件所属库' },
            { key: 'noteFileCreateTime', title: '笔记创建时间' },
            { key: 'noteFileUpdateTime', title: '笔记修改时间' },
        ];
    return columns;
});
// 打开文件浏览器并获取文件信息
const openFileBrowser = () => __awaiter(void 0, void 0, void 0, function* () {
    try {
        const filePath = yield (0, plugin_dialog_1.open)({
            filters: [{ name: 'All Files', extensions: ['*'] }],
        });
        if (filePath) {
            // 更新文件路径
            currentForm.value.filePath = filePath;
            // 调用后台接口获取文件信息
            const fileInfo = yield (0, core_1.invoke)('get_file_info', { filePath });
            console.log('File Info:', fileInfo);
            // 根据当前数据库类型更新表单数据
            if (store.currentDatabase === 'importFileDB') {
                currentForm.value.importFileName = fileInfo.file_name;
                currentForm.value.type = fileInfo.file_type;
                currentForm.value.importFileCreateTime = fileInfo.create_time;
                currentForm.value.importFileSize = fileInfo.file_size;
            }
            else if (store.currentDatabase === 'NotesDb') {
                currentForm.value.noteFileName = fileInfo.file_name;
                currentForm.value.noteFileType = fileInfo.file_type;
                currentForm.value.noteFileCreateTime = fileInfo.create_time;
            }
        }
    }
    catch (error) {
        console.error('Error opening file browser:', error);
    }
});
// 显示添加文件对话框
const showAddFileModal = () => {
    isAddFileModalVisible.value = true;
};
const createImportFileList = (form) => {
    return {
        import_file_name: form.importFileName,
        description: form.description,
        library: form.library,
        tags: form.importedFileTags.split(','),
        file_type: form.type,
        import_file_create_time: form.importFileCreateTime,
        import_file_size: form.importFileSize,
        converted_markdown_filename: `${form.importFileName.replace(/\.[^/.]+$/, '')}.md`,
    };
};
// 提取创建 importNoteList 的逻辑
const createImportNoteList = (addedNoteInfo, currentForm) => {
    if (store.currentDatabase === 'NotesDb') {
        return {
            description: currentForm.description,
            note_file_name: currentForm.noteFileName,
            tags: currentForm.notesTags.split(','),
            note_file_type: currentForm.noteFileType,
            from_imported_file: currentForm.fromImportedFile,
            from_imported_file_library: currentForm.fromImportedFileLibrary,
            note_file_create_time: currentForm.noteFileCreateTime,
            note_file_update_time: currentForm.noteFileCreateTime,
        };
    }
    else {
        return {
            description: `${addedNoteInfo.from_filename}的笔记`,
            note_file_name: addedNoteInfo.note_file_name,
            tags: currentForm.importedFileTags.split(','), //导入文件时创建的笔记使用导入文件的标签
            note_file_type: 'md',
            from_imported_file: addedNoteInfo.from_filename,
            from_imported_file_library: addedNoteInfo.library,
            note_file_create_time: addedNoteInfo.create_time,
            note_file_update_time: addedNoteInfo.update_time,
        };
    }
};
// 提取添加文件到数据库的逻辑
const addFileToDatabase = (fileInfo) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        console.log('Adding info into table imported_files database:', fileInfo);
        const fileId = yield (0, core_1.invoke)('add_imported_file', {
            file: fileInfo,
        });
        console.log('File info successfully added to database, id: ', fileId);
        return fileId;
    }
    catch (error) {
        console.error('Error adding info into table imported_files database:', error);
        throw error;
    }
});
// 提取添加笔记到数据库的逻辑
const addNoteToDatabase = (noteInfo) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        console.log('Adding info into table notes database:', noteInfo);
        const noteId = yield (0, core_1.invoke)('add_note', {
            note: noteInfo,
        });
        console.log('Note info successfully added to database, id: ', noteId);
        return noteId;
    }
    catch (error) {
        console.error('Error adding info into table notes database:', error);
        throw error;
    }
});
// 构建文件路径
const buildConvertOutputFilePath = (fileName, library) => __awaiter(void 0, void 0, void 0, function* () {
    const fileNameWithoutExt = fileName.replace(/\.[^/.]+$/, "");
    const osPlatform = yield (0, plugin_os_1.platform)(); // 使用Tauri API获取操作系统类型
    const basePath = osPlatform === 'windows'
        ? `${store.librariesRoot}\\Libraries\\${library}\\${fileNameWithoutExt}\\markdown`
        : `${store.librariesRoot}/Libraries/${library}/${fileNameWithoutExt}/markdown`;
    const fullPath = osPlatform === 'windows'
        ? `${basePath}\\${fileNameWithoutExt}\\${fileNameWithoutExt}.md`
        : `${basePath}/${fileNameWithoutExt}/${fileNameWithoutExt}.md`;
    console.log('basePath:', basePath);
    console.log('fullPath:', fullPath);
    return {
        outputDir: basePath,
        fileToCheck: fullPath
    };
});
const addFile = () => __awaiter(void 0, void 0, void 0, function* () {
    try {
        // 校验表单
        const validationError = yield validateForm(); // 等待 validateForm 的结果
        if (validationError) {
            message.error(validationError); // 确保 validationError 是字符串
            return; // 终止添加
        }
        if (store.currentDatabase === 'importFileDB') {
            if (!currentForm.value.library || !currentForm.value.importFileName) {
                message.error('Library or importFileName path is missing: ', currentForm.value);
                return;
            }
            // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
            const addedNoteInfo = yield (0, core_1.invoke)('add_repo_import_file', {
                library: currentForm.value.library,
                filePath: currentForm.value.filePath,
            });
            // 创建 importfileList
            const importfileList = createImportFileList(currentForm.value);
            console.log('importfileList:', importfileList);
            // 将 importfileList 写入数据库
            const fileId = yield addFileToDatabase(importfileList);
            // 添加导入的文件到 store
            store.addImportedFile({
                id: fileId,
                importFileName: currentForm.value.importFileName,
                description: currentForm.value.description,
                importedFileTags: currentForm.value.importedFileTags,
                type: currentForm.value.type,
                library: currentForm.value.library,
                importFileCreateTime: currentForm.value.importFileCreateTime,
                importFileSize: currentForm.value.importFileSize,
                convetToMarkdownFilename: `${currentForm.value.importFileName.replace(/\.[^/.]+$/, "")}.md`,
            });
            // 创建 importNoteList
            const importNoteList = createImportNoteList(addedNoteInfo, currentForm.value);
            console.log('importNoteList:', importNoteList);
            // 将 importNoteList 写入数据库
            const noteId = yield addNoteToDatabase(importNoteList);
            // 添加导入的笔记到 store
            store.addImportedNote({
                id: noteId,
                description: currentForm.value.description,
                noteFileName: `${currentForm.value.noteFileName}.md`,
                type: 'md',
                notesTags: currentForm.value.notesTags,
                fromImportedFileLibrary: currentForm.value.fromImportedFileLibrary,
                noteFileCreateTime: currentForm.value.noteFileCreateTime,
                noteFileUpdateTime: currentForm.value.noteFileUpdateTime || currentForm.value.noteFileCreateTime,
            });
            try {
                // 构建输出目录路径
                const { outputDir, fileToCheck } = yield buildConvertOutputFilePath(currentForm.value.importFileName, currentForm.value.library);
                // 检查文件是否已转换
                const isConverted = yield (0, core_1.invoke)('check_file_converted', {
                    outputDir: fileToCheck
                });
                if (isConverted && (!currentForm.value.convetToMarkdownFilename ||
                    currentForm.value.convetToMarkdownFilename.trim() === '')) {
                    console.log('isConverted', isConverted);
                    console.log('convetToMarkdownFilename', currentForm.value.convetToMarkdownFilename);
                    message.success('已存在转换文件，无需转换');
                    // 关闭对话框并清空表单
                    isAddFileModalVisible.value = false;
                    resetForm();
                    return;
                }
                // 开始监听进度事件
                const unlisten = yield startProgressListener(currentForm.value.filePath);
                // 立即关闭表单对话框
                isAddFileModalVisible.value = false;
                // 开始转换流程
                yield convertFile(currentForm.value.filePath, outputDir);
                // 转换完成后刷新数据
                yield store.fetchFiles('importFileDB');
                resetForm();
                // 转换完成后取消监听
                unlisten();
            }
            catch (error) {
                // 更新进度条状态为失败
                const fileName = currentForm.value.filePath.split(/[\\/]/).pop() || '未知文件';
                const progressIndex = progresses.value.findIndex(p => p.fileName === fileName);
                if (progressIndex !== -1) {
                    progresses.value[progressIndex] = Object.assign(Object.assign({}, progresses.value[progressIndex]), { message: `转换失败: ${error.message || error}`, current: 100 });
                    // 3秒后自动移除失败的进度条
                    setTimeout(() => {
                        progresses.value = progresses.value.filter((_, i) => i !== progressIndex);
                    }, 3000);
                }
                message.error(`添加文件失败: ${error}`);
            }
        }
        else if (store.currentDatabase === 'NotesDb') {
            // 创建 importNoteList
            const importNoteList = createImportNoteList(null, currentForm.value);
            console.log('importNoteList:', importNoteList);
            // 将 importNoteList 写入数据库
            const noteId = yield addNoteToDatabase(importNoteList);
            // 添加导入的笔记到 store
            store.addImportedNote({
                id: noteId,
                description: currentForm.value.description,
                noteFileName: `${currentForm.value.noteFileName}.md`,
                type: 'md',
                notesTags: currentForm.value.notesTags,
                fromImportedFile: currentForm.value.fromImportedFile,
                fromImportedFileLibrary: currentForm.value.fromImportedFileLibrary,
                noteFileCreateTime: currentForm.value.noteFileCreateTime,
                noteFileUpdateTime: currentForm.value.noteFileUpdateTime || currentForm.value.noteFileCreateTime,
            });
        }
        // 关闭对话框并清空表单
        isAddFileModalVisible.value = false;
        resetForm();
    }
    catch (error) {
        console.error('Error adding file:', error);
    }
});
// 更新导入文件
const updateImportedFile = (row) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        // 将 tags 字段转换为数组
        const importedFileTags = typeof row.importedFileTags === 'string' ? row.importedFileTags.split(',') : row.importedFileTags;
        // 确保 file 是一个对象
        const file = {
            import_file_name: row.importFileName,
            description: row.description,
            library: row.library,
            file_type: row.type,
            import_file_create_time: row.importFileCreateTime,
            import_file_size: row.importFileSize,
            tags: importedFileTags, // 传递转换后的 tags 数组
        };
        // 打印日志：row.id 和 file
        console.log('row.id:', row.id);
        console.log('file:', JSON.stringify(file, null, 2));
        // 调用后端接口
        yield (0, core_1.invoke)('update_imported_file', {
            id: row.id,
            file: file, // 传递正确的 file 对象
        });
        console.log('更新导入文件成功:', row);
    }
    catch (error) {
        console.error('更新导入文件失败:', error);
    }
});
// 更新笔记
const updateNote = (row) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        // 将 tags 字段转换为数组
        const notesTags = typeof row.notesTags === 'string' ? row.notesTags.split(',') : row.notesTags;
        // 确保 note 是一个对象
        const note = {
            description: row.description,
            note_file_name: row.noteFileName,
            note_file_type: row.noteFileType,
            from_imported_file: row.fromImportedFile,
            from_imported_file_library: row.fromImportedFileLibrary,
            note_file_create_time: row.noteFileCreateTime,
            note_file_update_time: row.noteFileUpdateTime,
            tags: notesTags, // 传递转换后的 tags 数组
        };
        // 打印日志：row.id 和 note
        console.log('row.id:', row.id);
        console.log('note:', JSON.stringify(note, null, 2));
        // 调用后端接口
        yield (0, core_1.invoke)('update_note', {
            id: row.id,
            note: note, // 传递正确的 note 对象
        });
        console.log('更新笔记成功:', row);
    }
    catch (error) {
        console.error('更新笔记失败:', error);
    }
});
// 清空表单
const resetForm = () => {
    if (store.currentDatabase === 'importFileDB') {
        newFileForm.value = {
            id: 0,
            importFileName: '',
            description: '',
            importedFileTags: '',
            type: '',
            library: '', // 所属库名字
            importFileCreateTime: '',
            importFileSize: 0,
            convetToHtmlFilename: '',
            convetToMarkdownFilename: "",
            importFileSize: '',
        };
    }
    else {
        newNoteForm.value = {
            id: 0,
            description: '',
            noteFileName: '',
            notesTags: '', // 所属库名字
            fromImportedFile: '',
            fromImportedFileLibrary: '',
            noteFileType: '',
            noteFileCreateTime: '',
            noteFileUpdateTime: '', // 笔记特有的字段
        };
    }
};
// 根据当前数据库类型选择数据
const currentData = (0, vue_1.computed)(() => {
    return store.currentDatabase === 'NotesDb' ? store.notes : store.files;
});
// 右键菜单状态
const showDropdown = (0, vue_1.ref)(false);
const x = (0, vue_1.ref)(0);
const y = (0, vue_1.ref)(0);
const selectedRow = (0, vue_1.ref)(null);
// 右键菜单选项
const contextMenuOptions = [
    {
        label: '打开',
        key: 'open',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.Open20Regular, size: 16 }),
    },
    {
        label: '编辑',
        key: 'edit',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.Edit20Regular, size: 16 }),
    },
    {
        label: '分享',
        key: 'share',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.Share20Regular, size: 16 }),
    },
    {
        label: '删除',
        key: 'delete',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.Delete20Regular, size: 16 }),
    },
    {
        label: '移动',
        key: 'move',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.ArrowMove24Regular, size: 16 }),
    },
];
// 处理右键菜单点击
const handleContextMenuSelect = (key) => {
    if (!selectedRow.value)
        return;
    switch (key) {
        case 'open':
            handleOpen(selectedRow.value);
            break;
        case 'edit':
            handleEdit(selectedRow.value);
            break;
        case 'share':
            handleShare(selectedRow.value);
            break;
        case 'delete':
            handleDelete(selectedRow.value);
            break;
        case 'move':
            handleMove(selectedRow.value);
            break;
    }
    showDropdown.value = false;
};
// 处理点击外部关闭菜单
const onClickoutside = () => {
    showDropdown.value = false;
};
// 行属性配置：右键点击触发菜单
const rowProps = (row) => {
    return {
        onContextmenu: (e) => {
            e.preventDefault();
            selectedRow.value = row;
            showDropdown.value = false;
            (0, vue_1.nextTick)(() => {
                showDropdown.value = true;
                x.value = e.clientX;
                y.value = e.clientY;
            });
        },
    };
};
// 方法：打开文件
const handleOpen = (row) => __awaiter(void 0, void 0, void 0, function* () {
    var _a, _b;
    // 1. 确定文件类型和文件名
    const rowFileType = store.currentDatabase === 'importFileDB'
        ? ((_a = row.type) === null || _a === void 0 ? void 0 : _a.toLowerCase()) || ''
        : ((_b = row.noteFileType) === null || _b === void 0 ? void 0 : _b.toLowerCase()) || '';
    const rowFileName = store.currentDatabase === 'importFileDB'
        ? row.importFileName || ''
        : row.noteFileName || '';
    // 2. 映射文件类型
    const fileType = mapFileType(rowFileType);
    if (!fileType) {
        console.warn('未知文件类型:', rowFileType, row);
        return;
    }
    // 获取不带后缀的文件名
    const baseFileName = rowFileName.includes('.')
        ? rowFileName.substring(0, rowFileName.lastIndexOf('.'))
        : rowFileName;
    // 3. 准备窗口参数
    const windowLabel = `reader-${row.id}-${Date.now()}`; // 唯一窗口标识
    const windowTitle = `${rowFileName || '未命名文件'}`; // 使用不带后缀的文件名
    const osPlatform = yield (0, plugin_os_1.platform)();
    let ReadFilePath = '';
    console.log('store.librariesRoot:', store.librariesRoot);
    if (store.currentDatabase === 'importFileDB') {
        if (osPlatform === 'windows') {
            ReadFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\${rowFileType}\\${rowFileName}`;
        }
        else if (osPlatform === 'linux') {
            ReadFilePath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/${rowFileType}/${rowFileName}`;
        }
        else {
            console.error('未知的操作系统平台:', osPlatform);
        }
    }
    else if (store.currentDatabase === 'NotesDb') {
        if (osPlatform === 'windows') {
            ReadFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\notes\\${rowFileName}`;
        }
        else if (osPlatform === 'linux') {
            ReadFilePath = `${store.librariesRoot}/Library/${row.library}/${baseFileName}/notes/${rowFileName}`;
        }
        else {
            console.error('未知的操作系统平台:', osPlatform);
        }
    }
    else {
        console.error('未知的数据库类型:', store.currentDatabase);
    }
    try {
        // 4. 调用 Rust 命令创建窗口
        yield (0, core_1.invoke)('create_reader_window', {
            label: windowLabel,
            title: windowTitle,
            fileData: {
                file_type: fileType,
                file_name: rowFileName,
                file_path: ReadFilePath,
                library: row.library,
            }
        });
        console.log('窗口创建参数:', {
            label: windowLabel,
            title: windowTitle,
            fileData: {
                type: fileType,
                name: rowFileName,
                path: ReadFilePath,
                library: row.library,
            }
        });
    }
    catch (error) {
        console.error('创建窗口失败:', error);
        alert(`无法打开文件: ${error.message || '未知错误'}`);
    }
});
// 文件类型映射函数
function mapFileType(rawType) {
    const typeMap = {
        'pdf': 'pdf',
        'md': 'markdown',
        'markdown': 'markdown',
        'html': 'html',
        'htm': 'html',
        'epub': 'epub',
        'doc': 'word',
        'docx': 'word',
        'word': 'word',
        'txt': 'txt'
    };
    return typeMap[rawType] || null;
}
const handleDelete = (row) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        if (store.currentDatabase === 'importFileDB') {
            // 调用删除导入文件的接口
            yield (0, core_1.invoke)('delete_imported_file', { id: row.id });
            console.log('删除导入文件成功', row);
        }
        else {
            // 调用删除笔记的接口
            yield (0, core_1.invoke)('delete_note', { id: row.id });
            console.log('删除笔记成功', row);
        }
        // 刷新数据
        yield store.fetchFiles(store.currentDatabase);
    }
    catch (error) {
        console.error('删除失败:', error);
    }
});
// 方法：编辑文件或笔记
const handleEdit = (row) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        if (store.currentDatabase === 'importFileDB') {
            // 调用更新导入文件的接口
            const updatedFile = {
                id: row.id,
                import_file_name: row.importFileName,
                description: row.description,
                library: row.library,
                file_type: row.type,
                import_file_create_time: row.importFileCreateTime,
                import_file_size: row.importFileSize,
                tags: row.importedFileTags.split(','), // 将字符串转换为数组
            };
            yield (0, core_1.invoke)('update_imported_file', { id: row.id, file: updatedFile });
            console.log('更新导入文件成功', row);
        }
        else {
            // 调用更新笔记的接口
            const updatedNote = {
                id: row.id,
                description: row.description,
                note_file_name: row.noteFileName,
                note_file_type: row.type,
                from_imported_file: row.fromImportedFile,
                from_imported_file_library: row.fromImportedFileLibrary,
                note_file_create_time: row.noteFileCreateTime,
                note_file_update_time: row.noteFileUpdateTime,
                tags: row.notesTags.split(','), // 将字符串转换为数组
            };
            yield (0, core_1.invoke)('update_note', { id: row.id, note: updatedNote });
            console.log('更新笔记成功', row);
        }
        // 刷新数据
        yield store.fetchFiles(store.currentDatabase);
    }
    catch (error) {
        console.error('编辑失败:', error);
    }
});
// 方法：分享文件
const handleShare = (row) => {
    console.log('分享', row);
};
// 方法：移动文件
const handleMove = (row) => {
    console.log('移动', row);
};
// 表格列配置
const columns = (0, vue_1.computed)(() => {
    return store.currentDatabase === 'NotesDb'
        ? [
            {
                type: 'selection',
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记描述'),
                key: 'description',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.description,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            console.log('更新笔记描述', v);
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { description: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.description = v;
                            yield updateNote(row); // 调用更新笔记的接口
                        }),
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记文件名'),
                key: 'noteFileName',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.noteFileName,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { noteFileName: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.noteFileName = v;
                            yield updateNote(row); // 调用更新笔记的接口
                        }),
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记标签'),
                key: 'notesTags',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.notesTags,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            console.log('更新笔记标签', v);
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { notesTags: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.notesTags = v;
                            yield updateNote(row); // 调用更新笔记的接口
                        }),
                        enableAutoComplete: true, // 启用自动补全
                        searchType: 'noteTags', // 指定搜索类型
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记归属导入文件'),
                key: 'fromImportedFile',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.fromImportedFile,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { fromImportedFile: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.fromImportedFile = v;
                            yield updateNote(row); // 调用更新笔记的接口
                        }),
                        enableAutoComplete: true, // 启用自动补全
                        searchType: 'fromImportedFile', // 指定搜索类型
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '导入文件所属库'),
                key: 'fromImportedFileLibrary',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.fromImportedFileLibrary,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { fromImportedFileLibrary: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.fromImportedFileLibrary = v;
                            yield updateNote(row); // 调用更新笔记的接口
                        }),
                        enableAutoComplete: true, // 启用自动补全
                        searchType: 'importedFileLibraries', // 指定搜索类型
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记文件类型'),
                key: 'noteFileType',
                resizable: true,
                width: 130,
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记创建时间'),
                key: 'noteFileCreateTime',
                resizable: true,
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '笔记修改时间'),
                key: 'noteFileUpdateTime',
                resizable: true,
            },
        ]
        : [
            {
                type: 'selection',
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '文件名'),
                key: 'importFileName',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.importFileName,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { importFileName: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.importFileName = v; // 更新前端表格数据
                            yield updateImportedFile(row); // 调用更新导入文件的接口
                        }),
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '文件描述'),
                key: 'description',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.description,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { description: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.description = v; // 更新前端表格数据
                            yield updateImportedFile(row); // 调用更新导入文件的接口
                        }),
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '文件标签'),
                key: 'importedFileTags',
                resizable: true,
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.importedFileTags,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { importedFileTags: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.importedFileTags = v;
                            yield updateImportedFile(row); // 调用更新导入文件的接口
                        }),
                        enableAutoComplete: true, // 启用自动补全
                        searchType: 'importedFileTags', // 指定搜索类型
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '文件类型'),
                key: 'type',
                resizable: true,
                width: 100,
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '所属库'),
                key: 'library',
                resizable: true,
                filterOptions: [
                    { label: 'Library1', value: 'Library1' },
                    { label: 'Library2', value: 'Library2' },
                ],
                filter(value, row) {
                    return row.library === value;
                },
                render(row) {
                    return (0, vue_1.h)(ShowOrEdit_vue_1.default, {
                        value: row.library,
                        onUpdateValue: (v) => __awaiter(this, void 0, void 0, function* () {
                            const error = validateTableCell(Object.assign(Object.assign({}, row), { library: v }));
                            if (error) {
                                message.error(error); // 使用 useMessage 的消息组件
                                return;
                            }
                            row.library = v;
                            yield updateImportedFile(row); // 调用更新导入文件的接口
                        }),
                        enableAutoComplete: true, // 启用自动补全
                        searchType: 'libraries', // 指定搜索类型
                    });
                },
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '入库时间'),
                key: 'importFileCreateTime',
                resizable: true,
                width: '170',
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, '文件大小'),
                key: 'importFileSize',
                resizable: true,
                width: 100,
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, 'HTML文件名'),
                key: 'convetToHtmlFilename',
                resizable: true,
                width: '180',
            },
            {
                title: () => (0, vue_1.h)('span', {
                    style: {
                        fontSize: '17px', // 调整字号
                        fontWeight: 'bold', // 加粗
                        color: '#000000', // 字体颜色
                    }
                }, 'Markdown文件名'),
                key: 'convetToMarkdownFilename',
                resizable: true,
                width: '180',
            },
        ];
});
// 方法：按标题排序
const sortTitle = () => {
    tableRef.value.sort('title', 'ascend');
};
// 方法：过滤所属库
const filterLibrary = () => {
    tableRef.value.filter({
        library: ['Library1'],
    });
};
// 方法：清除过滤器
const clearFilters = () => {
    tableRef.value.filter(null);
};
// 方法：清除排序
const clearSorter = () => {
    tableRef.value.sort(null);
};
// 选中的行
const checkedRowKeys = (0, vue_1.ref)([]);
// 定义 row-key 函数
const rowKey = (row) => row.id;
// 处理选中行
const handleCheckboxUpdate = (rowKeys) => {
    checkedRowKeys.value = rowKeys;
    console.log('选中的行 keys:', rowKeys);
};
// 引入搜索逻辑
const { noteTagOptions, isNoteTagLoading, handleNoteTagSearch, } = (0, useSearch_1.useNoteTagsSearch)();
const { importedFileTagOptions, isImportedFileTagLoading, handleImportedFileTagSearch, } = (0, useSearch_1.useImportedFileTagsSearch)();
const { importedFileOptions, isImportedFileLoading, handleImportedFileSearch, } = (0, useSearch_1.useImportedFilesSearch)();
const { libraryOptions, isLibraryLoading, handleLibrarySearch, } = (0, useSearch_1.useLibrariesSearch)();
const { importedFileLibraryOptions, isImportedFileLibraryLoading, handleImportedFileLibrarySearch, } = (0, useSearch_1.useImportedFileLibrarySearch)();
// 监听输入值变化并触发搜索
(0, vue_1.watch)(() => currentForm.value.importedFileTags, (newValue) => {
    handleImportedFileTagSearch(newValue);
});
(0, vue_1.watch)(() => currentForm.value.notesTags, (newValue) => {
    handleNoteTagSearch(newValue);
});
(0, vue_1.watch)(() => currentForm.value.fromImportedFile, (newValue) => {
    handleImportedFileSearch(newValue);
});
(0, vue_1.watch)(() => currentForm.value.library, (newValue) => {
    handleLibrarySearch(newValue);
});
(0, vue_1.watch)(() => currentForm.value.fromImportedFileLibrary, (newValue) => {
    handleImportedFileLibrarySearch(newValue);
});
// 组件挂载时获取默认数据
(0, vue_1.onMounted)(() => __awaiter(void 0, void 0, void 0, function* () {
    store.get_librariesRoot();
    yield store.fetchFiles('importFileDB'); // 默认加载数据库 1 的数据
})); /* PartiallyEnd: #3632/scriptSetup.vue */
const __VLS_fnComponent = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({});
;
let __VLS_functionalComponentProps;
function __VLS_template() {
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign(Object.assign({}, {}), {}), __VLS_ctx);
    let __VLS_components;
    const __VLS_localDirectives = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_directives;
    let __VLS_styleScopedClasses;
    __VLS_styleScopedClasses['actions'];
    __VLS_styleScopedClasses['add-path-button'];
    __VLS_styleScopedClasses['custom-dropdown'];
    __VLS_styleScopedClasses['custom-dropdown'];
    __VLS_styleScopedClasses['n-dropdown-option'];
    __VLS_styleScopedClasses['custom-dropdown'];
    __VLS_styleScopedClasses['n-dropdown-option'];
    __VLS_styleScopedClasses['custom-form'];
    __VLS_styleScopedClasses['custom-form'];
    __VLS_styleScopedClasses['progress-container'];
    __VLS_styleScopedClasses['n-progress'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign({ vertical: (true), size: ((12)) }, { class: ("container") })));
    const __VLS_2 = __VLS_1(Object.assign({ vertical: (true), size: ((12)) }, { class: ("container") }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6(Object.assign({ justify: ("space-between"), align: ("center") }, { class: ("toolbar") })));
    const __VLS_8 = __VLS_7(Object.assign({ justify: ("space-between"), align: ("center") }, { class: ("toolbar") }), ...__VLS_functionalComponentArgsRest(__VLS_7));
    const __VLS_12 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_13 = __VLS_asFunctionalComponent(__VLS_12, new __VLS_12(Object.assign({ class: ("actions") })));
    const __VLS_14 = __VLS_13(Object.assign({ class: ("actions") }), ...__VLS_functionalComponentArgsRest(__VLS_13));
    const __VLS_18 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_19 = __VLS_asFunctionalComponent(__VLS_18, new __VLS_18(Object.assign({ 'onClick': {} }, { class: ("action-button") })));
    const __VLS_20 = __VLS_19(Object.assign({ 'onClick': {} }, { class: ("action-button") }), ...__VLS_functionalComponentArgsRest(__VLS_19));
    let __VLS_24;
    const __VLS_25 = {
        onClick: (__VLS_ctx.sortTitle)
    };
    let __VLS_21;
    let __VLS_22;
    __VLS_nonNullable(__VLS_23.slots).default;
    var __VLS_23;
    const __VLS_26 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_27 = __VLS_asFunctionalComponent(__VLS_26, new __VLS_26(Object.assign({ 'onClick': {} }, { class: ("action-button") })));
    const __VLS_28 = __VLS_27(Object.assign({ 'onClick': {} }, { class: ("action-button") }), ...__VLS_functionalComponentArgsRest(__VLS_27));
    let __VLS_32;
    const __VLS_33 = {
        onClick: (__VLS_ctx.filterLibrary)
    };
    let __VLS_29;
    let __VLS_30;
    __VLS_nonNullable(__VLS_31.slots).default;
    var __VLS_31;
    const __VLS_34 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_35 = __VLS_asFunctionalComponent(__VLS_34, new __VLS_34(Object.assign({ 'onClick': {} }, { class: ("action-button") })));
    const __VLS_36 = __VLS_35(Object.assign({ 'onClick': {} }, { class: ("action-button") }), ...__VLS_functionalComponentArgsRest(__VLS_35));
    let __VLS_40;
    const __VLS_41 = {
        onClick: (__VLS_ctx.clearFilters)
    };
    let __VLS_37;
    let __VLS_38;
    __VLS_nonNullable(__VLS_39.slots).default;
    var __VLS_39;
    const __VLS_42 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_43 = __VLS_asFunctionalComponent(__VLS_42, new __VLS_42(Object.assign({ 'onClick': {} }, { class: ("action-button") })));
    const __VLS_44 = __VLS_43(Object.assign({ 'onClick': {} }, { class: ("action-button") }), ...__VLS_functionalComponentArgsRest(__VLS_43));
    let __VLS_48;
    const __VLS_49 = {
        onClick: (__VLS_ctx.clearSorter)
    };
    let __VLS_45;
    let __VLS_46;
    __VLS_nonNullable(__VLS_47.slots).default;
    var __VLS_47;
    __VLS_nonNullable(__VLS_17.slots).default;
    var __VLS_17;
    const __VLS_50 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
    /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
    // @ts-ignore
    const __VLS_51 = __VLS_asFunctionalComponent(__VLS_50, new __VLS_50({ trigger: ("hover"), }));
    const __VLS_52 = __VLS_51({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_51));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_55.slots);
        const __VLS_56 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_57 = __VLS_asFunctionalComponent(__VLS_56, new __VLS_56(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("add-path-button") })));
        const __VLS_58 = __VLS_57(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("add-path-button") }), ...__VLS_functionalComponentArgsRest(__VLS_57));
        let __VLS_62;
        const __VLS_63 = {
            onClick: (__VLS_ctx.showAddFileModal)
        };
        let __VLS_59;
        let __VLS_60;
        const __VLS_64 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_65 = __VLS_asFunctionalComponent(__VLS_64, new __VLS_64({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }));
        const __VLS_66 = __VLS_65({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_65));
        __VLS_nonNullable(__VLS_61.slots).default;
        var __VLS_61;
    }
    var __VLS_55;
    __VLS_nonNullable(__VLS_11.slots).default;
    var __VLS_11;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("main-content") }));
    const __VLS_70 = __VLS_resolvedLocalAndGlobalComponents.NDataTable;
    /** @type { [typeof __VLS_components.NDataTable, typeof __VLS_components.nDataTable, ] } */
    // @ts-ignore
    const __VLS_71 = __VLS_asFunctionalComponent(__VLS_70, new __VLS_70(Object.assign(Object.assign(Object.assign({ 'onUpdate:checkedRowKeys': {} }, { ref: ("table"), columns: ((__VLS_ctx.columns)), data: ((__VLS_ctx.currentData)), headerHeight: ((28)), virtualScroll: (true), maxHeight: ((500)), scrollX: ((1500)), pagination: ((__VLS_ctx.pagination)), singleLine: ((false)) }), { class: ("data-table") }), { rowProps: ((__VLS_ctx.rowProps)), multiple: ((true)), rowKey: ((__VLS_ctx.rowKey)) })));
    const __VLS_72 = __VLS_71(Object.assign(Object.assign(Object.assign({ 'onUpdate:checkedRowKeys': {} }, { ref: ("table"), columns: ((__VLS_ctx.columns)), data: ((__VLS_ctx.currentData)), headerHeight: ((28)), virtualScroll: (true), maxHeight: ((500)), scrollX: ((1500)), pagination: ((__VLS_ctx.pagination)), singleLine: ((false)) }), { class: ("data-table") }), { rowProps: ((__VLS_ctx.rowProps)), multiple: ((true)), rowKey: ((__VLS_ctx.rowKey)) }), ...__VLS_functionalComponentArgsRest(__VLS_71));
    // @ts-ignore navigation for `const table = ref()`
    __VLS_ctx.table;
    var __VLS_76 = {};
    let __VLS_77;
    const __VLS_78 = {
        'onUpdate:checkedRowKeys': (__VLS_ctx.handleCheckboxUpdate)
    };
    let __VLS_73;
    let __VLS_74;
    var __VLS_75;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("progress-container-wrapper") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("progress-container") }));
    for (const [progress, index] of __VLS_getVForSourceType((__VLS_ctx.progresses))) {
        const __VLS_79 = __VLS_resolvedLocalAndGlobalComponents.NCard;
        /** @type { [typeof __VLS_components.NCard, typeof __VLS_components.nCard, typeof __VLS_components.NCard, typeof __VLS_components.nCard, ] } */
        // @ts-ignore
        const __VLS_80 = __VLS_asFunctionalComponent(__VLS_79, new __VLS_79({ key: ((index)), }));
        const __VLS_81 = __VLS_80({ key: ((index)), }, ...__VLS_functionalComponentArgsRest(__VLS_80));
        const __VLS_85 = __VLS_resolvedLocalAndGlobalComponents.NText;
        /** @type { [typeof __VLS_components.NText, typeof __VLS_components.nText, typeof __VLS_components.NText, typeof __VLS_components.nText, ] } */
        // @ts-ignore
        const __VLS_86 = __VLS_asFunctionalComponent(__VLS_85, new __VLS_85({ strong: (true), }));
        const __VLS_87 = __VLS_86({ strong: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_86));
        (progress.fileName);
        __VLS_nonNullable(__VLS_90.slots).default;
        var __VLS_90;
        const __VLS_91 = __VLS_resolvedLocalAndGlobalComponents.NProgress;
        /** @type { [typeof __VLS_components.NProgress, typeof __VLS_components.nProgress, ] } */
        // @ts-ignore
        const __VLS_92 = __VLS_asFunctionalComponent(__VLS_91, new __VLS_91({ type: ("line"), percentage: ((progress.current)), indicatorPlacement: (('inside')), status: ("info"), }));
        const __VLS_93 = __VLS_92({ type: ("line"), percentage: ((progress.current)), indicatorPlacement: (('inside')), status: ("info"), }, ...__VLS_functionalComponentArgsRest(__VLS_92));
        const __VLS_97 = __VLS_resolvedLocalAndGlobalComponents.NText;
        /** @type { [typeof __VLS_components.NText, typeof __VLS_components.nText, typeof __VLS_components.NText, typeof __VLS_components.nText, ] } */
        // @ts-ignore
        const __VLS_98 = __VLS_asFunctionalComponent(__VLS_97, new __VLS_97({ depth: ("3"), }));
        const __VLS_99 = __VLS_98({ depth: ("3"), }, ...__VLS_functionalComponentArgsRest(__VLS_98));
        (progress.message);
        __VLS_nonNullable(__VLS_102.slots).default;
        var __VLS_102;
        __VLS_nonNullable(__VLS_84.slots).default;
        var __VLS_84;
    }
    const __VLS_103 = __VLS_resolvedLocalAndGlobalComponents.NDropdown;
    /** @type { [typeof __VLS_components.NDropdown, typeof __VLS_components.nDropdown, ] } */
    // @ts-ignore
    const __VLS_104 = __VLS_asFunctionalComponent(__VLS_103, new __VLS_103(Object.assign(Object.assign({ 'onSelect': {} }, { placement: ("bottom-start"), trigger: ("manual"), x: ((__VLS_ctx.x)), y: ((__VLS_ctx.y)), options: ((__VLS_ctx.contextMenuOptions)), show: ((__VLS_ctx.showDropdown)), onClickoutside: ((__VLS_ctx.onClickoutside)) }), { class: ("custom-dropdown") })));
    const __VLS_105 = __VLS_104(Object.assign(Object.assign({ 'onSelect': {} }, { placement: ("bottom-start"), trigger: ("manual"), x: ((__VLS_ctx.x)), y: ((__VLS_ctx.y)), options: ((__VLS_ctx.contextMenuOptions)), show: ((__VLS_ctx.showDropdown)), onClickoutside: ((__VLS_ctx.onClickoutside)) }), { class: ("custom-dropdown") }), ...__VLS_functionalComponentArgsRest(__VLS_104));
    let __VLS_109;
    const __VLS_110 = {
        onSelect: (__VLS_ctx.handleContextMenuSelect)
    };
    let __VLS_106;
    let __VLS_107;
    var __VLS_108;
    const __VLS_111 = __VLS_resolvedLocalAndGlobalComponents.NModal;
    /** @type { [typeof __VLS_components.NModal, typeof __VLS_components.nModal, typeof __VLS_components.NModal, typeof __VLS_components.nModal, ] } */
    // @ts-ignore
    const __VLS_112 = __VLS_asFunctionalComponent(__VLS_111, new __VLS_111({ show: ((__VLS_ctx.isAddFileModalVisible)), title: ("添加文件"), preset: ("dialog"), maskClosable: ((false)), closable: ((false)), }));
    const __VLS_113 = __VLS_112({ show: ((__VLS_ctx.isAddFileModalVisible)), title: ("添加文件"), preset: ("dialog"), maskClosable: ((false)), closable: ((false)), }, ...__VLS_functionalComponentArgsRest(__VLS_112));
    const __VLS_117 = __VLS_resolvedLocalAndGlobalComponents.NForm;
    /** @type { [typeof __VLS_components.NForm, typeof __VLS_components.nForm, typeof __VLS_components.NForm, typeof __VLS_components.nForm, ] } */
    // @ts-ignore
    const __VLS_118 = __VLS_asFunctionalComponent(__VLS_117, new __VLS_117(Object.assign({ model: ((__VLS_ctx.currentForm)), rules: ((__VLS_ctx.rules)), labelPlacement: ("left"), labelWidth: ("auto"), labelAlign: ("right"), size: ("medium") }, { class: ("custom-form") })));
    const __VLS_119 = __VLS_118(Object.assign({ model: ((__VLS_ctx.currentForm)), rules: ((__VLS_ctx.rules)), labelPlacement: ("left"), labelWidth: ("auto"), labelAlign: ("right"), size: ("medium") }, { class: ("custom-form") }), ...__VLS_functionalComponentArgsRest(__VLS_118));
    for (const [column] of __VLS_getVForSourceType((__VLS_ctx.formColumns))) {
        const __VLS_123 = __VLS_resolvedLocalAndGlobalComponents.NFormItem;
        /** @type { [typeof __VLS_components.NFormItem, typeof __VLS_components.nFormItem, typeof __VLS_components.NFormItem, typeof __VLS_components.nFormItem, ] } */
        // @ts-ignore
        const __VLS_124 = __VLS_asFunctionalComponent(__VLS_123, new __VLS_123({ key: ((column.key)), label: ((column.title)), path: ((column.key)), labelStyle: (({ fontWeight: 'bold', color: '#333', fontSize: '15px' })), }));
        const __VLS_125 = __VLS_124({ key: ((column.key)), label: ((column.title)), path: ((column.key)), labelStyle: (({ fontWeight: 'bold', color: '#333', fontSize: '15px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_124));
        if (column.key === 'filePath') {
            const __VLS_129 = __VLS_resolvedLocalAndGlobalComponents.NInput;
            /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
            // @ts-ignore
            const __VLS_130 = __VLS_asFunctionalComponent(__VLS_129, new __VLS_129({ value: ((__VLS_ctx.currentForm[column.key])), placeholder: ("请输入文件路径"), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_131 = __VLS_130({ value: ((__VLS_ctx.currentForm[column.key])), placeholder: ("请输入文件路径"), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_130));
            __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
            {
                const { suffix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_134.slots);
                const __VLS_135 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
                /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
                // @ts-ignore
                const __VLS_136 = __VLS_asFunctionalComponent(__VLS_135, new __VLS_135({ trigger: ("hover"), }));
                const __VLS_137 = __VLS_136({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_136));
                __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
                {
                    const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_140.slots);
                    const __VLS_141 = __VLS_resolvedLocalAndGlobalComponents.NButton;
                    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
                    // @ts-ignore
                    const __VLS_142 = __VLS_asFunctionalComponent(__VLS_141, new __VLS_141(Object.assign(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("FolderAdd20Regular") }), { style: (({ color: '#409EFF' })) })));
                    const __VLS_143 = __VLS_142(Object.assign(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("FolderAdd20Regular") }), { style: (({ color: '#409EFF' })) }), ...__VLS_functionalComponentArgsRest(__VLS_142));
                    let __VLS_147;
                    const __VLS_148 = {
                        onClick: (__VLS_ctx.openFileBrowser)
                    };
                    let __VLS_144;
                    let __VLS_145;
                    const __VLS_149 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
                    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
                    // @ts-ignore
                    const __VLS_150 = __VLS_asFunctionalComponent(__VLS_149, new __VLS_149({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((20)), }));
                    const __VLS_151 = __VLS_150({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((20)), }, ...__VLS_functionalComponentArgsRest(__VLS_150));
                    __VLS_nonNullable(__VLS_146.slots).default;
                    var __VLS_146;
                }
                var __VLS_140;
            }
            var __VLS_134;
        }
        else if (column.key === 'importFileCreateTime' || column.key === 'noteFileCreateTime' || column.key === 'noteFileUpdateTime'
            || column.key === 'noteFileType' || column.key === 'importFileName' || column.key === 'type' || column.key === 'importFileSize') {
            const __VLS_155 = __VLS_resolvedLocalAndGlobalComponents.NInput;
            /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
            // @ts-ignore
            const __VLS_156 = __VLS_asFunctionalComponent(__VLS_155, new __VLS_155({ value: ((__VLS_ctx.currentForm[column.key])), placeholder: ((`${column.title}`)), disabled: (true), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_157 = __VLS_156({ value: ((__VLS_ctx.currentForm[column.key])), placeholder: ((`${column.title}`)), disabled: (true), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_156));
        }
        else if (column.key === 'notesTags') {
            const __VLS_161 = __VLS_resolvedLocalAndGlobalComponents.NAutoComplete;
            /** @type { [typeof __VLS_components.NAutoComplete, typeof __VLS_components.nAutoComplete, ] } */
            // @ts-ignore
            const __VLS_162 = __VLS_asFunctionalComponent(__VLS_161, new __VLS_161({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.noteTagOptions)), loading: ((__VLS_ctx.isNoteTagLoading)), placeholder: ("请输入笔记标签"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_163 = __VLS_162({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.noteTagOptions)), loading: ((__VLS_ctx.isNoteTagLoading)), placeholder: ("请输入笔记标签"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_162));
        }
        else if (column.key === 'importedFileTags') {
            const __VLS_167 = __VLS_resolvedLocalAndGlobalComponents.NAutoComplete;
            /** @type { [typeof __VLS_components.NAutoComplete, typeof __VLS_components.nAutoComplete, ] } */
            // @ts-ignore
            const __VLS_168 = __VLS_asFunctionalComponent(__VLS_167, new __VLS_167({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.importedFileTagOptions)), loading: ((__VLS_ctx.isImportedFileTagLoading)), placeholder: ("请输入导入文件标签"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_169 = __VLS_168({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.importedFileTagOptions)), loading: ((__VLS_ctx.isImportedFileTagLoading)), placeholder: ("请输入导入文件标签"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_168));
        }
        else if (column.key === 'fromImportedFile') {
            const __VLS_173 = __VLS_resolvedLocalAndGlobalComponents.NAutoComplete;
            /** @type { [typeof __VLS_components.NAutoComplete, typeof __VLS_components.nAutoComplete, ] } */
            // @ts-ignore
            const __VLS_174 = __VLS_asFunctionalComponent(__VLS_173, new __VLS_173({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.importedFileOptions)), loading: ((__VLS_ctx.isImportedFileLoading)), placeholder: ("请输入导入文件"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_175 = __VLS_174({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.importedFileOptions)), loading: ((__VLS_ctx.isImportedFileLoading)), placeholder: ("请输入导入文件"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_174));
        }
        else if (column.key === 'library') {
            const __VLS_179 = __VLS_resolvedLocalAndGlobalComponents.NAutoComplete;
            /** @type { [typeof __VLS_components.NAutoComplete, typeof __VLS_components.nAutoComplete, ] } */
            // @ts-ignore
            const __VLS_180 = __VLS_asFunctionalComponent(__VLS_179, new __VLS_179({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.libraryOptions)), loading: ((__VLS_ctx.isLibraryLoading)), placeholder: ("请输入所属库"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_181 = __VLS_180({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.libraryOptions)), loading: ((__VLS_ctx.isLibraryLoading)), placeholder: ("请输入所属库"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_180));
        }
        else if (column.key === 'fromImportedFileLibrary') {
            const __VLS_185 = __VLS_resolvedLocalAndGlobalComponents.NAutoComplete;
            /** @type { [typeof __VLS_components.NAutoComplete, typeof __VLS_components.nAutoComplete, ] } */
            // @ts-ignore
            const __VLS_186 = __VLS_asFunctionalComponent(__VLS_185, new __VLS_185({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.importedFileLibraryOptions)), loading: ((__VLS_ctx.isImportedFileLibraryLoading)), placeholder: ("请输入导入文件所属库"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_187 = __VLS_186({ value: ((__VLS_ctx.currentForm[column.key])), options: ((__VLS_ctx.importedFileLibraryOptions)), loading: ((__VLS_ctx.isImportedFileLibraryLoading)), placeholder: ("请输入导入文件所属库"), clearable: (true), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_186));
        }
        else {
            const __VLS_191 = __VLS_resolvedLocalAndGlobalComponents.NInput;
            /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
            // @ts-ignore
            const __VLS_192 = __VLS_asFunctionalComponent(__VLS_191, new __VLS_191({ value: ((__VLS_ctx.currentForm[column.key])), placeholder: ((column.key === 'tags' ? `如有多个标签请以逗号隔开` : `请输入${column.title}`)), inputStyle: (({ borderRadius: '8px' })), }));
            const __VLS_193 = __VLS_192({ value: ((__VLS_ctx.currentForm[column.key])), placeholder: ((column.key === 'tags' ? `如有多个标签请以逗号隔开` : `请输入${column.title}`)), inputStyle: (({ borderRadius: '8px' })), }, ...__VLS_functionalComponentArgsRest(__VLS_192));
        }
        __VLS_nonNullable(__VLS_128.slots).default;
        var __VLS_128;
    }
    __VLS_nonNullable(__VLS_122.slots).default;
    var __VLS_122;
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { action: __VLS_thisSlot } = __VLS_nonNullable(__VLS_116.slots);
        const __VLS_197 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
        /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
        // @ts-ignore
        const __VLS_198 = __VLS_asFunctionalComponent(__VLS_197, new __VLS_197({ justify: ("end"), }));
        const __VLS_199 = __VLS_198({ justify: ("end"), }, ...__VLS_functionalComponentArgsRest(__VLS_198));
        const __VLS_203 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_204 = __VLS_asFunctionalComponent(__VLS_203, new __VLS_203(Object.assign(Object.assign({ 'onClick': {} }, { type: ("primary") }), { style: (({ backgroundColor: '#409EFF', borderColor: '#409EFF' })) })));
        const __VLS_205 = __VLS_204(Object.assign(Object.assign({ 'onClick': {} }, { type: ("primary") }), { style: (({ backgroundColor: '#409EFF', borderColor: '#409EFF' })) }), ...__VLS_functionalComponentArgsRest(__VLS_204));
        let __VLS_209;
        const __VLS_210 = {
            onClick: (__VLS_ctx.addFile)
        };
        let __VLS_206;
        let __VLS_207;
        __VLS_nonNullable(__VLS_208.slots).default;
        var __VLS_208;
        const __VLS_211 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_212 = __VLS_asFunctionalComponent(__VLS_211, new __VLS_211(Object.assign({ 'onClick': {} }, { style: (({ backgroundColor: '#F56C6C', borderColor: '#F56C6C', color: '#FFF' })) })));
        const __VLS_213 = __VLS_212(Object.assign({ 'onClick': {} }, { style: (({ backgroundColor: '#F56C6C', borderColor: '#F56C6C', color: '#FFF' })) }), ...__VLS_functionalComponentArgsRest(__VLS_212));
        let __VLS_217;
        const __VLS_218 = {
            onClick: (...[$event]) => {
                __VLS_ctx.isAddFileModalVisible = false;
            }
        };
        let __VLS_214;
        let __VLS_215;
        __VLS_nonNullable(__VLS_216.slots).default;
        var __VLS_216;
        __VLS_nonNullable(__VLS_202.slots).default;
        var __VLS_202;
    }
    var __VLS_116;
    __VLS_styleScopedClasses['container'];
    __VLS_styleScopedClasses['toolbar'];
    __VLS_styleScopedClasses['actions'];
    __VLS_styleScopedClasses['action-button'];
    __VLS_styleScopedClasses['action-button'];
    __VLS_styleScopedClasses['action-button'];
    __VLS_styleScopedClasses['action-button'];
    __VLS_styleScopedClasses['add-path-button'];
    __VLS_styleScopedClasses['main-content'];
    __VLS_styleScopedClasses['data-table'];
    __VLS_styleScopedClasses['progress-container-wrapper'];
    __VLS_styleScopedClasses['progress-container'];
    __VLS_styleScopedClasses['custom-dropdown'];
    __VLS_styleScopedClasses['custom-form'];
    __VLS_styleScopedClasses['FolderAdd20Regular'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "table": __VLS_76,
    };
    var $refs;
    var $el;
    return {
        attrs: {},
        slots: __VLS_slots,
        refs: $refs,
        rootEl: $el,
    };
}
;
const __VLS_self = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {
            FolderAdd20Regular: fluent_1.FolderAdd20Regular,
            NIcon: naive_ui_1.NIcon,
            NButton: naive_ui_1.NButton,
            NDropdown: naive_ui_1.NDropdown,
            isAddFileModalVisible: isAddFileModalVisible,
            pagination: pagination,
            currentForm: currentForm,
            progresses: progresses,
            rules: rules,
            formColumns: formColumns,
            openFileBrowser: openFileBrowser,
            showAddFileModal: showAddFileModal,
            addFile: addFile,
            currentData: currentData,
            showDropdown: showDropdown,
            x: x,
            y: y,
            contextMenuOptions: contextMenuOptions,
            handleContextMenuSelect: handleContextMenuSelect,
            onClickoutside: onClickoutside,
            rowProps: rowProps,
            columns: columns,
            sortTitle: sortTitle,
            filterLibrary: filterLibrary,
            clearFilters: clearFilters,
            clearSorter: clearSorter,
            rowKey: rowKey,
            handleCheckboxUpdate: handleCheckboxUpdate,
            noteTagOptions: noteTagOptions,
            isNoteTagLoading: isNoteTagLoading,
            importedFileTagOptions: importedFileTagOptions,
            isImportedFileTagLoading: isImportedFileTagLoading,
            importedFileOptions: importedFileOptions,
            isImportedFileLoading: isImportedFileLoading,
            libraryOptions: libraryOptions,
            isLibraryLoading: isLibraryLoading,
            importedFileLibraryOptions: importedFileLibraryOptions,
            isImportedFileLibraryLoading: isImportedFileLibraryLoading,
        };
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {};
    },
});
; /* PartiallyEnd: #4569/main.vue */
