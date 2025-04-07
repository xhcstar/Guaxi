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

  // 查找所有可能的代码块容器（包括 hljs 和 pre code）
  const codeContainers = Array.from(
    container.querySelectorAll(".hljs, pre code")
  );

  codeContainers.forEach((codeContainer) => {
    // 确保容器存在且没有重复按钮
    if (!codeContainer || codeContainer.querySelector(".copy-button")) return;

    // 获取实际的代码内容容器（可能是pre或.hljs）
    const codeBlock = codeContainer.classList.contains("hljs")
      ? codeContainer
      : codeContainer.parentElement;

    if (!codeBlock) return;

    // 创建复制按钮
    const copyButton = document.createElement("button");
    copyButton.className = "copy-button";

    // 创建图标元素
    const textSpan = document.createElement("span");
    textSpan.className = "copy-button-text";
    textSpan.textContent = "复制";

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
      background: rgba(255, 255, 255, 0.8);
      border: none;
      cursor: pointer;
      z-index: 10;
      gap: 4px;
    `;

    // 确保代码块容器有相对定位
    if (!codeBlock.style.position) {
      codeBlock.style.position = "relative";
    }

    // 添加按钮到代码块
    codeBlock.appendChild(copyButton);

    copyButton.onclick = async (e) => {
      e.stopPropagation();

      if (copyButton.dataset.isCopying === "true") {
        return;
      }

      try {
        const code = codeBlock.querySelector("code")?.textContent || "";
        await import("@tauri-apps/plugin-clipboard-manager").then(
          ({ writeText }) => writeText(code)
        );

        // 保存原始文本
        const originalText = textSpan.textContent;
        copyButton.dataset.originalText = originalText;
        copyButton.dataset.isCopying = "true";

        // 只修改文本内容
        textSpan.textContent = "已复制";

        // 清除之前的定时器（如果有）
        if (copyButton.dataset.timeoutId) {
          clearTimeout(parseInt(copyButton.dataset.timeoutId));
        }

        // 设置新的定时器
        const timeoutId = setTimeout(() => {
          // 恢复原始文本
          textSpan.textContent = originalText;
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 2000);

        copyButton.dataset.timeoutId = timeoutId.toString();
      } catch (err) {
        console.error("复制失败:", err);

        // 保存原始文本
        const originalText = textSpan.textContent;
        copyButton.dataset.originalText = originalText;
        copyButton.dataset.isCopying = "true";

        // 只修改文本内容
        textSpan.textContent = "复制失败";

        // 清除之前的定时器（如果有）
        if (copyButton.dataset.timeoutId) {
          clearTimeout(parseInt(copyButton.dataset.timeoutId));
        }

        // 设置新的定时器
        const timeoutId = setTimeout(() => {
          // 恢复原始文本
          textSpan.textContent = originalText;
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 2000);

        copyButton.dataset.timeoutId = timeoutId.toString();
      }
    };
  });
};

// 辅助函数：转义正则表达式特殊字符
function escapeRegExp(string: string) {
  return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

interface ImageMapping {
  original_path: string;
  served_path: string;
}

export const renderMarkdown = async (
  content: string,
  options: RenderOptions = {}
): Promise<string> => {
  console.log("原始内容:", content);
  const {
    mermaid: enableMermaid = true,
    math: enableMath = true,
    codeHighlight: enableCodeHighlight = true,
    theme = "default",
  } = options;

  try {
    // 调用后端处理图片路径
    const response = await fetch("http://localhost:8888/api/markdown/process", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        content,
        base_path: options.basePath,
      }),
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const result = await response.json();
    const processedContent = result.processed_content;
    const mappings = result.mappings || [];

    // 根据mappings替换图片路径
    let finalContent = processedContent;
    mappings.forEach((mapping: ImageMapping) => {
      finalContent = finalContent.replace(
        new RegExp(`\\]\\(${escapeRegExp(mapping.original_path)}\\)`, 'g'),
        `](${mapping.served_path})`
      );
    });

    console.log("result:", result);
    console.log("处理后的内容:", processedContent);
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
    container.innerHTML = md.render(finalContent);

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
  } catch (err) {
    console.error("Markdown处理失败:", err);
    return content;
  }
};
