// 静态预览页（http://localhost:8888/browser/preview）的图表入口。
//
// 为什么要打这个包：预览页是后端直接吐出的静态 HTML，没有打包器、不能 import ES 模块。
// 之前那里放了一份「精简版」的图表解析与 option 组装，结果和软件内模块各改各的：
// 软件内新增的图表类型（树图 / 旭日 / 关系 / 热力 / K线 / 甘特…）在浏览器里全部报
// 「静态预览暂不支持该图表类型」，直接贴完整 option 的写法也会报 spec 非法。
//
// 现在把**软件内同一份纯逻辑模块**打进一个 IIFE，两边共用一套解析语义，
// 从源头消除漂移。这里引用的模块都不碰 DOM / Tauri，Node 测试也在用它们。
//
// 产物：src-tauri/assets/vendor/guaxi-chart-preview.js
// 用法：node ./tools/build_chart_preview_bundle.mjs
import {
  parseChartFence,
  normalizeDataset,
  buildChartOption,
  buildLightOption,
} from '../src/components/GuaxiMarkdownRender/guaxiChartData.js'
import {
  CHART_TYPES,
  EXTENSION_BUILDERS,
  normalizeChartType,
  supportedChartTypes,
} from '../src/components/GuaxiMarkdownRender/guaxiChartTypes.js'
import { isBundledRef } from '../src/components/GuaxiMarkdownRender/guaxiChartSamples.js'
import {
  getBundledSample,
  listBundledSamples,
} from '../src/components/GuaxiMarkdownRender/guaxiChartSamples.bundled.js'

// 供页面脚本取用的唯一入口。名字与 excalidraw 导出器保持同一套约定。
globalThis.GuaxiChartPreview = {
  parseChartFence,
  normalizeDataset,
  buildChartOption,
  buildLightOption,
  CHART_TYPES,
  EXTENSION_BUILDERS,
  normalizeChartType,
  supportedChartTypes,
  isBundledRef,
  getBundledSample,
  listBundledSamples,
}
