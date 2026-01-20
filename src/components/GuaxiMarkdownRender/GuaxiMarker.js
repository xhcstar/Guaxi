import MarkdownIt from "markdown-it";
import markdownItMathjax from "markdown-it-mathjax3";
import mermaid from "mermaid";
import hljs from "highlight.js";
import "highlight.js/styles/atom-one-light.css";
import { initMathJax, renderMath } from "./mathjaxConfig";
import { nextTick } from "vue";

const RenderOptions = {
  mermaid: undefined,
  math: undefined,
  codeHighlight: undefined,
  theme: "default", // 移除类型注解
  basePath: "",
};

export const addCopyButtons = (container) => {
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

    const iconSvgMap = {
      copy: `
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/>
        </svg>
      `,
      check: `
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
        </svg>
      `,
      close: `
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M18.3 5.71 12 12l6.3 6.29-1.41 1.42L12 13.41l-6.29 6.3-1.42-1.41L10.59 12 4.29 5.71 5.7 4.29 12 10.59l6.29-6.3z"/>
        </svg>
      `,
    };

    const setCopyButtonState = (state) => {
      const nextState = state || "copy";
      const iconHtml = iconSvgMap[nextState] || iconSvgMap.copy;
      iconSpan.innerHTML = iconHtml;

      const titleMap = {
        copy: "复制",
        check: "已复制",
        close: "复制失败",
      };
      const title = titleMap[nextState] || "复制";
      copyButton.title = title;
      copyButton.setAttribute("aria-label", title);
    };

    // 创建图标元素（替代文案）
    const iconSpan = document.createElement("span");
    iconSpan.className = "copy-button-icon";
    iconSpan.style.cssText = `
      display: inline-flex;
      align-items: center;
      justify-content: center;
      line-height: 0;
    `;

    copyButton.appendChild(iconSpan);
    setCopyButtonState("copy");

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

        // 保存原始状态
        copyButton.dataset.isCopying = "true";

        // 只修改图标状态
        setCopyButtonState("check");

        // 清除之前的定时器（如果有）
        if (copyButton.dataset.timeoutId) {
          clearTimeout(parseInt(copyButton.dataset.timeoutId));
        }

        // 设置新的定时器
        const timeoutId = setTimeout(() => {
          // 恢复原始状态
          setCopyButtonState("copy");
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 2000);

        copyButton.dataset.timeoutId = timeoutId.toString();
      } catch (err) {
        console.error("复制失败:", err);

        // 保存原始状态
        copyButton.dataset.isCopying = "true";

        // 只修改图标状态
        setCopyButtonState("close");

        // 清除之前的定时器（如果有）
        if (copyButton.dataset.timeoutId) {
          clearTimeout(parseInt(copyButton.dataset.timeoutId));
        }

        // 设置新的定时器
        const timeoutId = setTimeout(() => {
          // 恢复原始状态
          setCopyButtonState("copy");
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 2000);

        copyButton.dataset.timeoutId = timeoutId.toString();
      }
    };
  });
};

// 辅助函数：转义正则表达式特殊字符
function escapeRegExp(string) {
  return string.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// interface ImageMapping {
//   original_path: string;
//   served_path: string;
// }

export const renderMarkdown = async (content, options) => {
  console.log("原始内容:", content);
  const {
    preserveOriginal = false, // 新增参数，控制是否保留原始内容
    mermaid: enableMermaid = true,
    math: enableMath = true,
    codeHighlight: enableCodeHighlight = true,
    theme = "default",
  } = options;

  try {
    let finalContent = content;
    let mappings = [];

    // 调用后端处理图片路径（接口不可用时，不影响后续 Markdown 本地渲染）
    try {
      const response = await fetch(
        "http://localhost:8888/api/markdown/image_process",
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            content,
            base_path: options.basePath,
          }),
        }
      );

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();
      finalContent = result.processed_content;
      mappings = result.mappings || [];

      // 根据mappings替换图片路径
      mappings.forEach((mapping) => {
        finalContent = finalContent.replace(
          new RegExp(`\\]\\(${escapeRegExp(mapping.original_path)}\\)`, "g"),
          `](${mapping.served_path})`
        );
      });
    } catch (err) {
      console.warn("[MarkdownRender] 图片路径处理失败，继续本地渲染:", err);
    }

    // console.log("result:", result);
    // console.log("处理后的内容:", finalContent);

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
      breaks: false, // 禁用单换行转换为 <br>
      xhtmlOut: true, // 使用XHTML兼容的闭合标签
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

    // 段落渲染规则
    const defaultParagraphOpen =
      md.renderer.rules.paragraph_open || ((tokens, idx) => "<p>");
    const defaultParagraphClose =
      md.renderer.rules.paragraph_close || ((tokens, idx) => "</p>");

    md.renderer.rules.paragraph_open = (tokens, idx) => {
      const token = tokens[idx];
      const attrs = ['class="md-paragraph"'];

      if (token && Array.isArray(token.map)) {
        const startLine = token.map[0] + 1;
        const endLine = token.map[1] + 1;
        attrs.push(`data-md-start-line="${startLine}"`);
        attrs.push(`data-md-end-line="${endLine}"`);

        if (preserveOriginal) {
          const originalLines = content.split(/\r?\n|\r/);
          const original = originalLines[token.map[0]] || "";
          attrs.push(`data-md-original="${safeEncode(original)}"`);
        }
      }

      return `<p ${attrs.join(" ")}>`;
    };

    md.renderer.rules.paragraph_close = (tokens, idx) => {
      return defaultParagraphClose(tokens, idx);
    };

    // 标题渲染规则：为标题添加源码行号，便于编辑/预览联动
    const defaultHeadingOpen =
      md.renderer.rules.heading_open ||
      ((tokens, idx) => `<${tokens[idx].tag}>`);
    const defaultHeadingClose =
      md.renderer.rules.heading_close ||
      ((tokens, idx) => `</${tokens[idx].tag}>`);

    md.renderer.rules.heading_open = (tokens, idx, options, env, self) => {
      const token = tokens[idx];
      const tag = token.tag || "h1";
      const attrs = [`class="md-heading md-heading-${tag}"`];

      if (token && Array.isArray(token.map)) {
        const startLine = token.map[0] + 1;
        const endLine = token.map[1] + 1;
        attrs.push(`data-md-start-line="${startLine}"`);
        attrs.push(`data-md-end-line="${endLine}"`);
      }

      return `<${tag} ${attrs.join(" ")}>`;
    };

    md.renderer.rules.heading_close = (tokens, idx, options, env, self) => {
      return defaultHeadingClose(tokens, idx, options, env, self);
    };

    // 添加Mermaid支持（同时保留原始内容功能）
    if (enableMermaid) {
      const defaultFence = md.renderer.rules.fence;
      md.renderer.rules.fence = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        if (token.info.trim() === "mermaid") {
          const content = token.content
            .replace(/&gt;/g, ">")
            .replace(/&lt;/g, "<")
            .replace(/&amp;/g, "&");

          // 如果需要保留原始内容，添加data-md-original属性
          if (preserveOriginal) {
            const original = `\`\`\`mermaid\n${content}\n\`\`\``;
            return `<pre class="mermaid" data-md-original="${safeEncode(
              original
            )}">${content}</pre>`;
          }
          return `<pre class="mermaid">${content}</pre>`;
        }
        return defaultFence(tokens, idx, options, env, self);
      };
    }

    // 添加数学公式支持（完整修改部分）
    if (enableMath) {
      md.use(markdownItMathjax);

      // 保存原始渲染函数
      const originalInlineMath = md.renderer.rules.math_inline;
      const originalBlockMath = md.renderer.rules.math_block;

      // 重写行内数学公式渲染
      md.renderer.rules.math_inline = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        const originalLatex = `$${token.content}$`;

        // 创建容器元素并添加原始内容
        const container = document.createElement("span");
        container.className = "math-inline-container";
        container.setAttribute("data-md-original", safeEncode(originalLatex));

        // 使用原始渲染器生成内容
        const mathContent = originalInlineMath
          ? originalInlineMath(tokens, idx, options, env, self)
          : `<span class="math-inline">${token.content}</span>`;

        container.innerHTML = mathContent;
        return container.outerHTML;
      };

      // 重写块级数学公式渲染
      md.renderer.rules.math_block = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        const originalLatex = `$$${token.content}$$`;

        // 创建容器元素并添加原始内容
        const container = document.createElement("div");
        container.className = "math-block-container";
        container.setAttribute("data-md-original", safeEncode(originalLatex));

        // 使用原始渲染器生成内容
        const mathContent = originalBlockMath
          ? originalBlockMath(tokens, idx, options, env, self)
          : `<div class="math-block">${token.content}</div>`;

        container.innerHTML = mathContent;
        return container.outerHTML;
      };
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
              const content = el.textContent
                .replace(/&gt;/g, ">")
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

    if (enableMath) {
      // 处理已渲染的数学公式
      container
        .querySelectorAll(".math-inline-container, .math-block-container")
        .forEach((el) => {
          const mjxContainer = el.querySelector("mjx-container");
          if (mjxContainer) {
            // 将原始内容转移到mjx-container
            mjxContainer.setAttribute(
              "data-md-original",
              el.getAttribute("data-md-original")
            );

            // 保留容器类名
            mjxContainer.classList.add(
              el.classList.contains("math-inline-container")
                ? "math-inline"
                : "math-block"
            );

            // 替换容器
            el.replaceWith(mjxContainer);
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

// 安全编码函数（完整实现）
function safeEncode(content) {
  if (!content) {
    console.warn("安全编码: 空内容");
    return "";
  }

  const encoded = encodeURIComponent(content)
    .replace(/'/g, "%27")
    .replace(/"/g, "%22")
    .replace(/</g, "%3C")
    .replace(/>/g, "%3E")
    .replace(/&/g, "%26")
    .replace(/\//g, "%2F")
    .replace(/\\/g, "%5C");

  // console.log("安全编码结果:", { original: content, encoded });
  return encoded;
}
