<template>
    <n-dropdown :options="dropdownOptions" placement="bottom-start" trigger="click" @select="handleFileSelect">
        <n-icon size="18" style="cursor: pointer;">
            <svg t="1740559605848" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
                p-id="1912" width="32" height="32">
                <path
                    d="M859.5 193H446.939c-1.851-53.25-45.747-96-99.439-96h-183C109.635 97 65 141.635 65 196.5v632c0 54.864 44.635 99.5 99.5 99.5h695c54.864 0 99.5-44.636 99.5-99.5v-536c0-54.865-44.636-99.5-99.5-99.5zM896 828.5c0 20.126-16.374 36.5-36.5 36.5h-695c-20.126 0-36.5-16.374-36.5-36.5V384h768v444.5z m0-507.5H128V196.5c0-20.126 16.374-36.5 36.5-36.5h183c20.126 0 36.5 16.374 36.5 36.5v28c0 17.397 14.103 31.5 31.5 31.5h444c20.126 0 36.5 16.374 36.5 36.5V321z"
                    fill="#070102" p-id="1913"></path>
                <path
                    d="M384 640h95v96c0 17.674 14.327 32 32 32s32-14.326 32-32v-96h97c17.673 0 32-14.327 32-32s-14.327-32-32-32h-97v-96c0-17.673-14.327-32-32-32s-32 14.327-32 32v96h-95c-17.673 0-32 14.327-32 32s14.327 32 32 32z"
                    fill="#070102" p-id="1914"></path>
            </svg>
        </n-icon>
    </n-dropdown>
</template>

<script setup lang="ts">
import type { DropdownOption } from 'naive-ui'
import { ref, computed } from 'vue'
import { useFileProcessStore } from '../../stores/fileProcessStore'
import { open } from '@tauri-apps/plugin-dialog'



const selectedFile = ref(null);
const fileProcessStore = useFileProcessStore();
const supportFileTypeList = fileProcessStore.supportFileType;

// 动态生成 options
const dropdownOptions = computed(() => {
    const openFilelist = {
        label: "打开文件",
        key: "local",
        children: supportFileTypeList.map((filetype) => {
            return {
                label: `打开${filetype.type}`,
                key: filetype.id,
            };
        }),
    };

    console.log('openFilelist', openFilelist);
    return [openFilelist];
});

// 打开文件选择框的函数
const handleFileSelect = async () => {
    try {
        // 打开文件选择器并获取选择的文件路径
        const filePath = await open({
            multiple: false,  // 如果需要多选文件，可以将其设置为 true
            filters: [{
                name: 'Text Files',
                extensions: ['txt', 'md', 'docx', 'pdf']  // 文件过滤器，可以选择你想要的文件类型
            }],
        });

        // 打印所选择的文件路径
        if (filePath) {
            selectedFile.value = filePath;
        } else {
            console.log('未选择文件');
        }
    } catch (error) {
        console.error('打开文件选择框时发生错误:', error);
    }
};
</script>