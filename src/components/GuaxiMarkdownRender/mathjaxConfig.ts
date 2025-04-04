import { mathjax } from 'mathjax-full/js/mathjax';
import { TeX } from 'mathjax-full/js/input/tex';
import { SVG } from 'mathjax-full/js/output/svg';
import { liteAdaptor } from 'mathjax-full/js/adaptors/liteAdaptor';
import { RegisterHTMLHandler } from 'mathjax-full/js/handlers/html';
import { nextTick } from 'vue';

// 初始化适配器
const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);

// 配置TeX输入
const tex = new TeX({
  packages: [
    'base', 'ams', 'autoload', 'require',
    'newcommand', 'noundefined', 'textmacros'
  ],
  inlineMath: [['$', '$'], ['\\(', '\\)']], // 避免 `$` 被误解析
  displayMath: [['$$', '$$'], ['\\[', '\\]']], // 支持 `\[ \]`
  processEscapes: true,
  processEnvironments: true,
});

// 配置SVG输出
const svg = new SVG({
  fontCache: 'global',
  scale: 1,
  minScale: 0.5,
  mtextInheritFont: true
});

// 创建 MathJax 处理器
let mathjaxDocument: any = null;

export const initMathJax = () => {
  if (!mathjaxDocument) {
    mathjaxDocument = mathjax.document('', {
      InputJax: tex,
      OutputJax: svg
    });
  }
};

// 渲染数学公式
export const renderMath = async (element: HTMLElement, isBlock: boolean) => {
  console.log('renderMath', element, isBlock);
  if (!mathjaxDocument) initMathJax();

  await nextTick(); // Vue 组件更新后再渲染

  try {
    // 修复特殊字符
    let rawContent = element.textContent || '';
    rawContent = rawContent
      .replace(/&amp;/g, '&')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/\\begin\{aligned\}/g, '\\begin{align*}')
      .replace(/\\end\{aligned\}/g, '\\end{align*}');

    console.log('处理后数学公式:', rawContent);

    // 解析 LaTeX 并转换为 SVG
    const converted = mathjaxDocument.convert(rawContent, {
      display: isBlock
    });

    // 清理并插入 SVG
    element.innerHTML = adaptor.innerHTML(converted);
    const svgElement = element.querySelector('svg');
    if (svgElement) {
      svgElement.style.maxWidth = '100%';
      svgElement.style.height = 'auto';
      svgElement.style.verticalAlign = 'middle';
    }
  } catch (err) {
    console.error('数学公式渲染错误:', err);
    element.innerHTML = `<span class="math-error">${err instanceof Error ? err.message : String(err)}</span>`;
  }
};
