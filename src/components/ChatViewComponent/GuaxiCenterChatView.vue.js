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
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
const chatStore_1 = require("../../stores/chatStore");
const core_1 = require("@tauri-apps/api/core");
const event_1 = require("@tauri-apps/api/event");
const GuaxiMarker_1 = require("../GuaxiMarkdownRender/GuaxiMarker");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const chatStore = (0, chatStore_1.useChatStore)();
const messageInput = (0, vue_1.ref)(""); // 用户输入的消息
let unsubscribe = null;
const messageRefs = (0, vue_1.ref)([]);
// 当前激活的聊天会话
const activeChat = (0, vue_1.computed)(() => chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel));
// 当前会话的消息记录
const activeChatMessages = (0, vue_1.computed)(() => { var _a; return ((_a = activeChat.value) === null || _a === void 0 ? void 0 : _a.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic)) || []; });
// 滚动到最新消息
const messageContainer = (0, vue_1.ref)(null);
const scrollToBottom = () => {
    (0, vue_1.nextTick)(() => {
        if (messageContainer.value) {
            messageContainer.value.scrollTop = messageContainer.value.scrollHeight;
        }
    });
};
const askDeepSeek = (question) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        const request = {
            "modelid": chatStore.selectedModel,
            "question": question,
            "topicid": chatStore.selectedTopic,
            "sessionid": chatStore.sendChatSessionId,
        };
        console.log('Request:', request);
        // const response = await invoke('get_chat_answer', { request });
        const response = yield (0, core_1.invoke)('get_chat_answer_stream', { request });
        console.log('Response:', response);
        return response;
    }
    catch (error) {
        console.error('Error calling API:', error);
    }
});
// 发送消息
const sendMessage = () => __awaiter(void 0, void 0, void 0, function* () {
    if (!messageInput.value.trim() || !activeChat.value)
        return;
    let question = messageInput.value.trim();
    activeChatMessages.value.topic.push({
        msgid: activeChatMessages.value.topic.length + 1,
        sender: "me",
        content: question,
    });
    messageInput.value = ""; // 清空输入框
    scrollToBottom(); // 滚动到底部
    chatStore.accumulatedMessage = '';
    // 更新当前会话的消息记录
    try {
        let answer = yield askDeepSeek(question); // 等待 Promise 解析
        console.log('answer:', answer);
        // chatStore.updateChatSession(answer);
    }
    catch (error) {
        console.error('Error during askDeepSeek:', error);
    }
    scrollToBottom(); // 滚动到底部
});
const handleKeydown = (event) => {
    if (event.key === 'Enter') {
        if (event.shiftKey) {
            // Shift + Enter 换行，什么都不做
            return;
        }
        // 阻止默认行为并发送消息
        event.preventDefault();
        sendMessage();
    }
};
let startY = 0;
let startHeight = 0;
const startResize = (e) => {
    e.preventDefault(); // 防止选中文本
    const inputArea = document.querySelector('.input-area');
    startY = e.clientY; // 记录鼠标初始位置
    startHeight = inputArea.offsetHeight; // 记录初始高度
    // 添加监听器，实时跟踪鼠标位置
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', stopResize);
    // 禁用文本框的选中
    document.body.style.userSelect = 'none';
};
const onMouseMove = (e) => {
    let diff = startY - e.clientY;
    if (diff < 0) {
        diff = 0;
    }
    const newHeight = startHeight + diff;
    console.log('newHeight:', newHeight);
    if (newHeight > 85) { // 限制最小高度
        const inputArea = document.querySelector('.input-area');
        inputArea.style.height = newHeight + 'px'; // 修改 height
        // inputArea.style.height = `${Math.max(180, newHeight)}px`
    }
};
const stopResize = () => {
    // 移除鼠标移动和鼠标松开事件监听器
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', stopResize);
    // 恢复文本框的选中功能
    document.body.style.userSelect = '';
};
(0, vue_1.onMounted)(() => {
    unsubscribe = (0, event_1.listen)("modle-streaming-respond-output", (event) => {
        chatStore.collectChatSessionStream(event.payload);
    }).then((unsubFn) => {
        unsubscribe = unsubFn;
    }).catch((err) => {
        console.error('监听事件失败:', err);
        unsubscribe = null;
    });
});
(0, vue_1.onUnmounted)(() => {
    if (unsubscribe && typeof unsubscribe === 'function') {
        unsubscribe(); // 清理监听器
    }
});
const setMessageRef = (el, index) => {
    if (el) {
        messageRefs.value[index] = el;
        (0, vue_1.nextTick)(() => {
            (0, GuaxiMarker_1.addCopyButtons)(el);
        });
    }
};
// watch(activeChatMessages, () => {
//   nextTick(() => {
//     messageRefs.value.forEach(container => {
//       if (container) {
//         addCopyButtons(container);
//       }
//     });
//   });
// }, { deep: true });
; /* PartiallyEnd: #3632/scriptSetup.vue */
const __VLS_fnComponent = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({});
;
let __VLS_functionalComponentProps;
function __VLS_template() {
    var _a, _b, _c;
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign(Object.assign({}, {}), {}), __VLS_ctx);
    let __VLS_components;
    const __VLS_localDirectives = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_directives;
    let __VLS_styleScopedClasses;
    __VLS_styleScopedClasses['message-left'];
    __VLS_styleScopedClasses['message-bubble'];
    __VLS_styleScopedClasses['message-right'];
    __VLS_styleScopedClasses['message-bubble'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-header") }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NAvatar;
    /** @type { [typeof __VLS_components.NAvatar, typeof __VLS_components.nAvatar, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ src: (((_a = __VLS_ctx.activeChat) === null || _a === void 0 ? void 0 : _a.ModelAvatar)), size: ("medium"), round: (true), }));
    const __VLS_2 = __VLS_1({ src: (((_b = __VLS_ctx.activeChat) === null || _b === void 0 ? void 0 : _b.ModelAvatar)), size: ("medium"), round: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("chat-title") }));
    (((_c = __VLS_ctx.activeChat) === null || _c === void 0 ? void 0 : _c.name) || "未选择会话");
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("message-list") }, { ref: ("messageContainer") }));
    // @ts-ignore navigation for `const messageContainer = ref()`
    __VLS_ctx.messageContainer;
    for (const [message, index] of __VLS_getVForSourceType((__VLS_ctx.activeChatMessages.topic))) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign({ key: ((message.msgid)) }, { class: ("message-item") }), { class: ((message.sender === 'me' ? 'message-right' : 'message-left')) }));
        if (message.sender === 'me') {
            const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NAvatar;
            /** @type { [typeof __VLS_components.NAvatar, typeof __VLS_components.nAvatar, ] } */
            // @ts-ignore
            const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6({ src: ((__VLS_ctx.activeChat.MeAvatar)), size: ("medium"), round: (true), }));
            const __VLS_8 = __VLS_7({ src: ((__VLS_ctx.activeChat.MeAvatar)), size: ("medium"), round: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_7));
        }
        else {
            const __VLS_12 = __VLS_resolvedLocalAndGlobalComponents.NAvatar;
            /** @type { [typeof __VLS_components.NAvatar, typeof __VLS_components.nAvatar, ] } */
            // @ts-ignore
            const __VLS_13 = __VLS_asFunctionalComponent(__VLS_12, new __VLS_12({ src: ((__VLS_ctx.activeChat.ModelAvatar)), size: ("medium"), round: (true), }));
            const __VLS_14 = __VLS_13({ src: ((__VLS_ctx.activeChat.ModelAvatar)), size: ("medium"), round: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_13));
        }
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("message-bubble") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("message-content") }, { ref: ((el => __VLS_ctx.setMessageRef(el, index))) }));
        __VLS_asFunctionalDirective(__VLS_directives.vHtml)(null, Object.assign(Object.assign({}, __VLS_directiveBindingRestFields), { modifiers: {}, value: (message.content) }), null, null);
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("input-area") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onMousedown: (__VLS_ctx.startResize) }, { class: ("chat-input-height-handle") }));
    const __VLS_18 = __VLS_resolvedLocalAndGlobalComponents.NInput;
    /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
    // @ts-ignore
    const __VLS_19 = __VLS_asFunctionalComponent(__VLS_18, new __VLS_18(Object.assign(Object.assign({ 'onKeydown': {} }, { type: ("textarea"), placeholder: ("请输入您的问题..."), value: ((__VLS_ctx.messageInput)) }), { style: ({}) })));
    const __VLS_20 = __VLS_19(Object.assign(Object.assign({ 'onKeydown': {} }, { type: ("textarea"), placeholder: ("请输入您的问题..."), value: ((__VLS_ctx.messageInput)) }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_19));
    let __VLS_24;
    const __VLS_25 = {
        onKeydown: (__VLS_ctx.handleKeydown)
    };
    let __VLS_21;
    let __VLS_22;
    var __VLS_23;
    const __VLS_26 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_27 = __VLS_asFunctionalComponent(__VLS_26, new __VLS_26(Object.assign(Object.assign({ 'onClick': {} }, { color: ("#dbe7e4") }), { style: ({}) })));
    const __VLS_28 = __VLS_27(Object.assign(Object.assign({ 'onClick': {} }, { color: ("#dbe7e4") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_27));
    let __VLS_32;
    const __VLS_33 = {
        onClick: (__VLS_ctx.sendMessage)
    };
    let __VLS_29;
    let __VLS_30;
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { icon: __VLS_thisSlot } = __VLS_nonNullable(__VLS_31.slots);
        const __VLS_34 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_35 = __VLS_asFunctionalComponent(__VLS_34, new __VLS_34({}));
        const __VLS_36 = __VLS_35({}, ...__VLS_functionalComponentArgsRest(__VLS_35));
        __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1737646310254") }, { class: ("icon") }), { viewBox: ("0 0 1178 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("5954"), width: ("48"), height: ("48") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M1145.732473 24.529036A96.502796 96.502796 0 0 0 1047.167948 6.302655L62.5449 378.38417a96.762677 96.762677 0 0 0-8.229592 177.44733l153.746105 75.175158a32.918368 32.918368 0 0 1 8.229592 5.804029c15.974071 41.130635 79.05606 202.292036 107.123301 260.540222a120.758435 120.758435 0 0 0 64.520002 59.530271c-1.611268-0.155929-3.465091-0.485113-5.318915-0.814297a88.359831 88.359831 0 0 0 14.518733 4.192761 96.190937 96.190937 0 0 0 87.59751-26.455973l57.260636-57.260636a32.485232 32.485232 0 0 1 38.722396-5.318915l246.662532 140.024344a97.143838 97.143838 0 0 0 47.748959 12.751536 97.022559 97.022559 0 0 0 94.042581-73.56389l205.999684-830.478457a95.671174 95.671174 0 0 0-29.453277-95.290014z m-868.005397 592.3747L805.56445 314.903695l-297.997861 305.222576a48.112794 48.112794 0 0 0-10.811085 17.325457c-0.155929 0.329184-0.155929 0.485113-0.329184 0.814297-0.641042 1.940451-59.53027 175.680134-94.371764 254.233756a58.404116 58.404116 0 0 1-20.651945-22.748325c-25.814931-54.211355-85.986243-207.628277-103.727511-252.795743z"), fill: ("#6EC2B0"), "p-id": ("5955"), });
        __VLS_nonNullable(__VLS_39.slots).default;
        var __VLS_39;
    }
    var __VLS_31;
    __VLS_styleScopedClasses['chat-container'];
    __VLS_styleScopedClasses['chat-header'];
    __VLS_styleScopedClasses['chat-title'];
    __VLS_styleScopedClasses['message-list'];
    __VLS_styleScopedClasses['message-item'];
    __VLS_styleScopedClasses['message-bubble'];
    __VLS_styleScopedClasses['message-content'];
    __VLS_styleScopedClasses['input-area'];
    __VLS_styleScopedClasses['chat-input-height-handle'];
    __VLS_styleScopedClasses['icon'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "messageContainer": __VLS_nativeElements['div'],
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
            messageInput: messageInput,
            activeChat: activeChat,
            activeChatMessages: activeChatMessages,
            messageContainer: messageContainer,
            sendMessage: sendMessage,
            handleKeydown: handleKeydown,
            startResize: startResize,
            setMessageRef: setMessageRef,
        };
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {};
    },
    __typeEl: {},
});
; /* PartiallyEnd: #4569/main.vue */
