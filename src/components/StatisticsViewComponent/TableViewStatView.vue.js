"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
const columns = [
    {
        title: 'Name',
        key: 'name',
        defaultSortOrder: 'ascend',
        sorter: 'default'
    },
    {
        title: 'Age',
        key: 'age',
        sorter: (row1, row2) => row1.age - row2.age
    },
    {
        title: 'Address',
        key: 'address',
        defaultFilterOptionValues: ['London', 'New York'],
        filterOptions: [
            {
                label: 'London',
                value: 'London'
            },
            {
                label: 'New York',
                value: 'New York'
            }
        ],
        filter(value, row) {
            return ~row.address.indexOf(value);
        }
    }
];
const data = [
    {
        key: 0,
        name: 'John Brown',
        age: 32,
        address: 'New York No. 1 Lake Park'
    },
    {
        key: 1,
        name: 'Jim Green',
        age: 42,
        address: 'London No. 1 Lake Park'
    },
    {
        key: 2,
        name: 'Joe Black',
        age: 32,
        address: 'Sidney No. 1 Lake Park'
    },
    {
        key: 3,
        name: 'Jim Red',
        age: 32,
        address: 'London No. 2 Lake Park'
    }
];
exports.default = (0, vue_1.defineComponent)({
    setup() {
        const tableRef = (0, vue_1.ref)(null);
        return {
            table: tableRef,
            data,
            columns,
            pagination: { pageSize: 5 },
            filterAddress() {
                tableRef.value.filter({
                    address: ['London']
                });
            },
            sortName() {
                tableRef.value.sort('name', 'ascend');
            },
            clearFilters() {
                tableRef.value.filter(null);
            },
            clearSorter() {
                tableRef.value.sort(null);
            }
        };
    }
}); /* PartiallyEnd: #3632/script.vue */
function __VLS_template() {
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_components;
    const __VLS_localDirectives = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_directives;
    let __VLS_styleScopedClasses;
    let __VLS_resolvedLocalAndGlobalComponents;
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ vertical: (true), size: ((12)), }));
    const __VLS_2 = __VLS_1({ vertical: (true), size: ((12)), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    var __VLS_6 = {};
    const __VLS_7 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_8 = __VLS_asFunctionalComponent(__VLS_7, new __VLS_7({}));
    const __VLS_9 = __VLS_8({}, ...__VLS_functionalComponentArgsRest(__VLS_8));
    const __VLS_13 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_14 = __VLS_asFunctionalComponent(__VLS_13, new __VLS_13(Object.assign({ 'onClick': {} })));
    const __VLS_15 = __VLS_14(Object.assign({ 'onClick': {} }), ...__VLS_functionalComponentArgsRest(__VLS_14));
    let __VLS_19;
    const __VLS_20 = {
        onClick: (__VLS_ctx.sortName)
    };
    let __VLS_16;
    let __VLS_17;
    __VLS_nonNullable(__VLS_18.slots).default;
    var __VLS_18;
    const __VLS_21 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_22 = __VLS_asFunctionalComponent(__VLS_21, new __VLS_21(Object.assign({ 'onClick': {} })));
    const __VLS_23 = __VLS_22(Object.assign({ 'onClick': {} }), ...__VLS_functionalComponentArgsRest(__VLS_22));
    let __VLS_27;
    const __VLS_28 = {
        onClick: (__VLS_ctx.filterAddress)
    };
    let __VLS_24;
    let __VLS_25;
    __VLS_nonNullable(__VLS_26.slots).default;
    var __VLS_26;
    const __VLS_29 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_30 = __VLS_asFunctionalComponent(__VLS_29, new __VLS_29(Object.assign({ 'onClick': {} })));
    const __VLS_31 = __VLS_30(Object.assign({ 'onClick': {} }), ...__VLS_functionalComponentArgsRest(__VLS_30));
    let __VLS_35;
    const __VLS_36 = {
        onClick: (__VLS_ctx.clearFilters)
    };
    let __VLS_32;
    let __VLS_33;
    __VLS_nonNullable(__VLS_34.slots).default;
    var __VLS_34;
    const __VLS_37 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_38 = __VLS_asFunctionalComponent(__VLS_37, new __VLS_37(Object.assign({ 'onClick': {} })));
    const __VLS_39 = __VLS_38(Object.assign({ 'onClick': {} }), ...__VLS_functionalComponentArgsRest(__VLS_38));
    let __VLS_43;
    const __VLS_44 = {
        onClick: (__VLS_ctx.clearSorter)
    };
    let __VLS_40;
    let __VLS_41;
    __VLS_nonNullable(__VLS_42.slots).default;
    var __VLS_42;
    __VLS_nonNullable(__VLS_12.slots).default;
    var __VLS_12;
    const __VLS_45 = __VLS_resolvedLocalAndGlobalComponents.NDataTable;
    /** @type { [typeof __VLS_components.NDataTable, typeof __VLS_components.nDataTable, ] } */
    // @ts-ignore
    const __VLS_46 = __VLS_asFunctionalComponent(__VLS_45, new __VLS_45({ ref: ("table"), columns: ((__VLS_ctx.columns)), data: ((__VLS_ctx.data)), pagination: ((__VLS_ctx.pagination)), }));
    const __VLS_47 = __VLS_46({ ref: ("table"), columns: ((__VLS_ctx.columns)), data: ((__VLS_ctx.data)), pagination: ((__VLS_ctx.pagination)), }, ...__VLS_functionalComponentArgsRest(__VLS_46));
    // @ts-ignore navigation for `const table = ref()`
    __VLS_ctx.table;
    var __VLS_51 = {};
    var __VLS_50;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "table": __VLS_51,
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
let __VLS_self;
