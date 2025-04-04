import MarkdownIt from "markdown-it";
import markdownItMathjax from "markdown-it-mathjax3";
import mermaid from "mermaid";
import hljs from "highlight.js";
import "highlight.js/styles/github.css";
import { initMathJax, renderMath } from "./mathjaxConfig";
import { nextTick } from "vue";

type RenderOptions = {
  mermaid?: boolean;
  math?: boolean;
  codeHighlight?: boolean;
  theme?: "default" | "dark" | "neutral";
  basePath?: string;
};

export const addCopyButtons = (container: HTMLElement) => {
  // 移除已有按钮
  container.querySelectorAll(".copy-button").forEach((btn) => btn.remove());

  // 仅添加新按钮
  container.querySelectorAll(".hljs").forEach((codeBlock) => {
    if (codeBlock.querySelector(".copy-button")) return;

    const copyButton = document.createElement("button");
    copyButton.className = "copy-button";

    // 创建图标元素
    const iconWrapper = document.createElement("span");
    iconWrapper.innerHTML = `
<svg t="1743777604650" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg" p-id="2612" width="16" height="16"><path d="M833.33 767.96h-91.9c-21.73 0-39.34-17.6-39.34-39.34s17.62-39.34 39.34-39.34h91.9c8.82 0 15.98-7.18 15.98-15.98V193.8c0-8.8-7.17-15.98-15.98-15.98H353.84c-8.82 0-15.98 7.18-15.98 15.98v90.86c0 21.75-17.62 39.34-39.34 39.34s-39.34-17.6-39.34-39.34V193.8c0-52.21 42.47-94.67 94.67-94.67h479.49c52.19 0 94.67 42.45 94.67 94.67v479.49c-0.01 52.21-42.49 94.67-94.68 94.67z" fill="#333333" p-id="2613"></path><path d="M675.96 925.33H196.47c-52.19 0-94.67-42.45-94.67-94.67V351.17c0-52.21 42.47-94.67 94.67-94.67h479.49c52.19 0 94.67 42.45 94.67 94.67v479.49c-0.01 52.22-42.48 94.67-94.67 94.67zM196.47 335.19c-8.82 0-15.98 7.18-15.98 15.98v479.49c0 8.8 7.17 15.98 15.98 15.98h479.49c8.82 0 15.98-7.18 15.98-15.98V351.17c0-8.8-7.17-15.98-15.98-15.98H196.47z" fill="#333333" p-id="2614"></path></svg>
    `;

    const textSpan = document.createElement("span");
    textSpan.className = "copy-button-text";
    textSpan.textContent = "复制";

    copyButton.appendChild(iconWrapper);
    copyButton.appendChild(textSpan);
    copyButton.style.cssText = `
      position: absolute;
      right: 12px;
      top: 12px;
      height: 24px;
      padding: 0 6px;
      border-radius: 6px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: rgba(0, 0, 0, 0.6);
      background: transparent;
      border: none;
      cursor: pointer;
      z-index: 10;
      opacity: 0;
      transition: opacity 0.2s ease;
      gap: 4px;
    `;

    // 鼠标悬停显示按钮
    codeBlock.addEventListener("mouseenter", () => {
      copyButton.style.opacity = "1";
    });
    codeBlock.addEventListener("mouseleave", () => {
      copyButton.style.opacity = "0";
    });

    codeBlock.style.position = "relative";
    codeBlock.appendChild(copyButton);

    copyButton.onclick = async (e) => {
      e.stopPropagation();

      // 如果当前状态不是初始状态，直接恢复并返回
      if (copyButton.dataset.isCopying === "true") {
        const originalHTML = copyButton.dataset.originalHtml;
        if (originalHTML) {
          copyButton.innerHTML = originalHTML;
        }
        delete copyButton.dataset.isCopying;
        return;
      }

      try {
        const code = codeBlock.querySelector("code")?.textContent || "";
        await import("@tauri-apps/plugin-clipboard-manager").then(
          ({ writeText }) => writeText(code)
        );

        // 保存原始HTML结构
        copyButton.dataset.originalHtml = copyButton.innerHTML;
        copyButton.dataset.isCopying = "true";

        // 临时修改为"已复制"
        copyButton.innerHTML = '<span class="copy-button-text">已复制</span>';

        // 存储定时器ID以便取消
        const timeoutId = setTimeout(() => {
          // 恢复原始HTML结构（包含图标）
          copyButton.innerHTML = copyButton.dataset.originalHtml || "";
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 1500);

        copyButton.dataset.timeoutId = timeoutId.toString();
      } catch (err) {
        console.error("复制失败:", err);

        // 保存原始HTML结构
        copyButton.dataset.originalHtml = copyButton.innerHTML;
        copyButton.dataset.isCopying = "true";

        // 临时修改为"复制失败"
        copyButton.innerHTML = '<span class="copy-button-text">复制失败</span>';

        // 存储定时器ID以便取消
        const timeoutId = setTimeout(() => {
          // 恢复原始HTML结构（包含图标）
          copyButton.innerHTML = copyButton.dataset.originalHtml || "";
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 1500);

        copyButton.dataset.timeoutId = timeoutId.toString();
      }
    };
  });
};

export const renderMarkdown = async (
  content: string,
  options: RenderOptions = {}
): Promise<string> => {
  // console.log("原始内容:", content);
  const {
    mermaid: enableMermaid = true,
    math: enableMath = true,
    codeHighlight: enableCodeHighlight = true,
    theme = "default",
  } = options;

  // 初始化各组件
  if (enableMermaid) {
    // console.log("初始化Mermaid配置");
    mermaid.initialize({
      startOnLoad: false,
      theme: theme === "dark" ? "dark" : "default",
      securityLevel: "loose",
    });
  }

  if (enableMath) {
    // console.log("初始化MathJax");
    initMathJax();
  }
  // 配置Markdown-it
  const md = new MarkdownIt({
    html: true,
    linkify: true,
    typographer: true,
    highlight: enableCodeHighlight
      ? (code, lang) => {
          if (lang && hljs.getLanguage(lang)) {
            try {
              return `<pre class="hljs"><code class="language-${lang}">${
                hljs.highlight(code, { language: lang }).value
              }</code></pre>`;
            } catch (err) {
              console.warn(`代码高亮错误 (${lang}):`, err);
            }
          }
          return `<pre><code>${md.utils.escapeHtml(code)}</code></pre>`;
        }
      : undefined,
  });

  // 添加Mermaid支持
  if (enableMermaid) {
    const defaultFence = md.renderer.rules.fence!;
    md.renderer.rules.fence = (tokens, idx, options, env, self) => {
      const token = tokens[idx];
      if (token.info.trim() === "mermaid") {
        const content = token.content
          .replace(/&gt;/g, ">")
          .replace(/&lt;/g, "<")
          .replace(/&amp;/g, "&");
        return `<pre class="mermaid">${content}</pre>`;
      }
      return defaultFence(tokens, idx, options, env, self);
    };
  }

  // 添加数学公式支持
  if (enableMath) {
    md.use(markdownItMathjax);
  }

  // 执行渲染
  const container = document.createElement("div");
  container.innerHTML = md.render(content);

  // 处理Mermaid图表
  if (enableMermaid) {
    const mermaidElements = container.querySelectorAll(".mermaid");
    if (mermaidElements.length > 0) {
      try {
        // 处理每个图表
        mermaidElements.forEach((el) => {
          try {
            const content = el
              .textContent!.replace(/&gt;/g, ">")
              .replace(/&lt;/g, "<")
              .replace(/&amp;/g, "&");
            el.innerHTML = content;
          } catch (err) {
            console.error("Mermaid内容处理错误:", err);
          }
        });

        // Vue 的 nextTick() 只是 等 Vue 自己的 DOM 变更完成,
        // 但并不等浏览器真正渲染并解析DOM nextTick()确保Vue更新了Virtual DOM, 并把它提交到浏览器.
        // 但浏览器可能尚未完成布局和样式计算, Mermaid可能无法正确解析 .mermaid 元素.
        // Mermaid可能依赖某些浏览器渲染完成的属性但nextTick()时这些属性可能尚未计算完成,导致Mermaid找不到.mermaid或渲染失败。
        // 尝试requestAnimationFrame()因为它会等到浏览器完成一次帧渲染后再执行.
        nextTick().then(() => {
          requestAnimationFrame(() => {
            mermaid.run();
          });
        });

        // console.log("Mermaid渲染完成: ", container.innerHTML);
      } catch (err) {
        console.error("Mermaid渲染错误:", err);
      }
    }
  }

  // 处理数学公式
  if (enableMath) {
    // console.log("开始处理数学公式");
    container.querySelectorAll(".math-inline, .math-block").forEach((el) => {
      try {
        // 获取原始内容并修复可能的转义字符
        const rawContent = el.textContent || "";
        const isBlock = el.classList.contains("math-block");

        // 临时清空内容避免重复处理
        el.textContent = "";

        // 创建新的span元素用于渲染
        const mathSpan = document.createElement("span");
        mathSpan.textContent = rawContent;
        el.appendChild(mathSpan);

        // 调用MathJax渲染
        renderMath(mathSpan as HTMLElement, isBlock);
      } catch (err) {
        console.error("数学公式渲染错误:", err);
        el.innerHTML = `<span class="math-error">${
          err instanceof Error ? err.message : String(err)
        }</span>`;
      }
    });
  }

  addCopyButtons(container);
  // console.log("渲染后的HTML:", container.innerHTML);
  return container.innerHTML;
};
