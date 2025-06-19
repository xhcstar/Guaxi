import { defineStore } from "pinia";
import { ref } from "vue";

export const useOutlineStore = defineStore("outline", () => {
  const outline = ref([]);
  const jumpToLine = ref(null); // 用 ref 包裹

  function setOutline(data) {
    outline.value = data;
  }
  function setJumpToLine(fn) {
    jumpToLine.value = fn;
  }
  return { outline, setOutline, jumpToLine, setJumpToLine };
});
