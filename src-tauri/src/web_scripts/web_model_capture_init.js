(() => {
  try {
    const PROVIDER = __GUAXI_PROVIDER_JSON__;
    const ENDPOINT = __GUAXI_ENDPOINT_JSON__;

    if (window.__GUAXI_WEB_CAPTURE_INSTALLED__) return;
    window.__GUAXI_WEB_CAPTURE_INSTALLED__ = true;

    function textOf(el) {
      if (!el) return '';
      return ((el.innerText || el.textContent || '').trim());
    }

    function markdownOf(root) {
      if (!root) return '';

      try {
        const dm = root.getAttribute && (root.getAttribute('data-markdown') || root.getAttribute('data-md'));
        if (dm && typeof dm === 'string' && dm.trim().length > 0) return dm.trim();
      } catch (e) { }

      const skipEl = (el) => {
        if (!el || !el.tagName) return false;
        const tag = el.tagName.toLowerCase();
        if (tag === 'script' || tag === 'style' || tag === 'svg' || tag === 'path') return true;
        if (tag === 'button' || tag === 'textarea' || tag === 'input') return true;
        const role = (el.getAttribute && el.getAttribute('role')) || '';
        if (role && role.toString().toLowerCase().includes('button')) return true;
        const id = (el.getAttribute && el.getAttribute('id')) || '';
        if (id === 'yuanbao-send-btn') return true;
        return false;
      };

      const escInline = (s) => {
        return String(s || '').replace(/`/g, '\\`');
      };

      const normalize = (s) => {
        return String(s || '')
          .replace(/\u00A0/g, ' ')
          .replace(/\r\n/g, '\n')
          .replace(/\r/g, '\n')
          .replace(/\n{3,}/g, '\n\n')
          .trim();
      };

      const joinChildren = (el, ctx) => {
        let out = '';
        const nodes = Array.from(el.childNodes || []);
        for (const n of nodes) {
          out += walk(n, ctx);
        }
        return out;
      };

      const prefixLines = (s, prefix) => {
        return String(s || '')
          .split('\n')
          .map((line) => (line ? prefix + line : prefix.trimEnd()))
          .join('\n');
      };

      const walk = (node, ctx) => {
        if (!node) return '';
        const t = node.nodeType;
        if (t === 3) {
          return node.nodeValue || '';
        }
        if (t !== 1) return '';

        const el = node;
        if (skipEl(el)) return '';

        const tag = (el.tagName || '').toLowerCase();
        if (!tag) return '';

        if (tag === 'br') return '\n';

        if (tag === 'pre') {
          const codeEl = el.querySelector && el.querySelector('code');
          let code = (codeEl ? codeEl.textContent : el.textContent) || '';
          code = String(code)
            .replace(/\r\n/g, '\n')
            .replace(/\r/g, '\n')
            .replace(/\n+$/, '');
          if (!code.trim()) return '';
          return `\n\n\`\`\`\n${code}\n\`\`\`\n\n`;
        }

        if (tag === 'code') {
          const codeText = normalize(el.textContent || '');
          if (!codeText) return '';
          if (el.parentElement && (el.parentElement.tagName || '').toLowerCase() === 'pre') return codeText;
          return '`' + escInline(codeText) + '`';
        }

        if (tag === 'strong' || tag === 'b') {
          const inner = normalize(joinChildren(el, ctx));
          return inner ? `**${inner}**` : '';
        }

        if (tag === 'em' || tag === 'i') {
          const inner = normalize(joinChildren(el, ctx));
          return inner ? `*${inner}*` : '';
        }

        if (tag === 'a') {
          const href = (el.getAttribute && el.getAttribute('href')) || '';
          const inner = normalize(joinChildren(el, ctx));
          if (!href) return inner;
          return `[${inner || href}](${href})`;
        }

        if (tag === 'h1' || tag === 'h2' || tag === 'h3' || tag === 'h4' || tag === 'h5' || tag === 'h6') {
          const n = parseInt(tag.slice(1), 10) || 2;
          const inner = normalize(joinChildren(el, ctx));
          if (!inner) return '';
          return `\n\n${'#'.repeat(Math.min(6, Math.max(1, n)))} ${inner}\n\n`;
        }

        if (tag === 'ul') {
          let out = '\n';
          const items = Array.from(el.children || []).filter((c) => (c.tagName || '').toLowerCase() === 'li');
          for (const li of items) {
            const inner = normalize(joinChildren(li, ctx));
            if (!inner) continue;
            out += `- ${inner}\n`;
          }
          return out + '\n';
        }

        if (tag === 'ol') {
          let out = '\n';
          const items = Array.from(el.children || []).filter((c) => (c.tagName || '').toLowerCase() === 'li');
          let idx = 1;
          for (const li of items) {
            const inner = normalize(joinChildren(li, ctx));
            if (!inner) continue;
            out += `${idx}. ${inner}\n`;
            idx++;
          }
          return out + '\n';
        }

        if (tag === 'blockquote') {
          const inner = normalize(joinChildren(el, ctx));
          if (!inner) return '';
          return '\n\n' + prefixLines(inner, '> ') + '\n\n';
        }

        if (tag === 'p') {
          const inner = normalize(joinChildren(el, ctx));
          return inner ? `\n\n${inner}\n\n` : '';
        }

        if (tag === 'div' || tag === 'section' || tag === 'article') {
          return joinChildren(el, ctx);
        }

        return joinChildren(el, ctx);
      };

      let md = walk(root, {});
      md = normalize(md);
      if (!md) {
        md = textOf(root);
      }
      return md;
    }

    function getSessionId() {
      const v = (window.__GUAXI_SESSION_ID__ || '').toString().trim();
      if (v) return v;

      const k = `guaxi_web_session_${PROVIDER}`;
      let stored = null;
      try { stored = localStorage.getItem(k); } catch (e) { }
      if (!stored) {
        stored = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
        try { localStorage.setItem(k, stored); } catch (e) { }
      }
      return stored;
    }

    function post(payload) {
      return fetch(ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload || {})
      });
    }

    function findInput() {
      return (
        document.querySelector('#prompt-textarea') ||
        document.querySelector('textarea') ||
        document.querySelector('[contenteditable="true"]')
      );
    }

    function isReady() {
      const input = findInput();
      if (!input) return false;

      if (PROVIDER === 'yuanbao') {
        try {
          if (input.disabled) return false;
          const aria = (input.getAttribute && input.getAttribute('aria-disabled')) || '';
          if (aria.toString().toLowerCase() === 'true') return false;
          if (Object.prototype.hasOwnProperty.call(input, 'readOnly') && input.readOnly) return false;
        } catch (e) { }
        return true;
      }

      try {
        const style = window.getComputedStyle(input);
        if (style && (style.display === 'none' || style.visibility === 'hidden')) return false;
      } catch (e) { }

      // Avoid mis-detecting need_login when the window/tab is hidden and layout metrics become 0.
      try {
        if (document && document.visibilityState && document.visibilityState !== 'visible') return true;
      } catch (e) { }

      if (PROVIDER === 'chatgpt') {
        return true;
      }

      const rect = input.getBoundingClientRect?.();
      if (rect && (rect.width === 0 || rect.height === 0)) return false;

      return true;
    }

    let lastStatus = '';
    async function pushStatus() {
      try {
        const status = isReady() ? 'ready' : 'need_login';
        if (status === lastStatus) return;
        lastStatus = status;

        await post({
          provider: PROVIDER,
          model_id: window.__GUAXI_MODEL_ID__,
          topic_id: window.__GUAXI_TOPIC_ID__,
          session_id: getSessionId(),
          status,
          messages: []
        });
      } catch (e) { }
    }

    function collectChatgptMessages() {
      const nodes = Array.from(document.querySelectorAll('[data-message-author-role]'));
      const out = [];
      for (let i = 0; i < nodes.length; i++) {
        const n = nodes[i];
        const role = (n.getAttribute('data-message-author-role') || '').toLowerCase();
        const content = textOf(n);
        if (!content) continue;
        const message_id = (n.getAttribute('data-message-id') || n.id || (role + '-' + i)).toString();
        out.push({ role: role === 'user' ? 'user' : 'assistant', content, message_id });
      }
      return out;
    }

    function hashString(s) {
      let h = 2166136261;
      for (let i = 0; i < s.length; i++) {
        h ^= s.charCodeAt(i);
        h += (h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24);
      }
      return (h >>> 0).toString(16);
    }

    function looksLikeYuanbaoUi(content) {
      const BAD_EXACT = new Set([
        '搜索',
        '元宝',
        '全部收藏',
        '分组',
        '创建分组',
        '前往下载中心',
        '深度思考',
        '联网搜索',
        '工具',
        '下载电脑版'
      ]);
      if (BAD_EXACT.has(content)) return true;

      const BAD_INCLUDES = [
        '文件拖动到此处即可上传',
        '支持文件格式',
        '下载元宝电脑版',
        '内容由AI生成',
        '仅供参考'
      ];
      if (BAD_INCLUDES.some((k) => content.includes(k))) return true;

      if (/\bjpg\b|\bjpeg\b|\bpng\b|\bwebp\b|\bbmp\b|\bgif\b|\bpdf\b|\bxls\b|\bxlsx\b|\bppt\b|\bpptx\b|\bdoc\b|\bdocx\b|\bcsv\b|\btext\b/i.test(content) && content.includes('、')) {
        return true;
      }

      const oneLine = content.replace(/[\r\n]+/g, ' ').trim();
      const tokens = oneLine.split(/\s+/).filter(Boolean);
      if (tokens.length >= 6 && oneLine.length <= 220 && !/[。！？?!：:，,]/.test(oneLine)) {
        let shortCount = 0;
        for (const t of tokens) {
          if (t.length >= 2 && t.length <= 12) shortCount++;
        }
        if (shortCount / tokens.length >= 0.9) return true;
      }

      const BAD_TOKEN = [
        '帮我画',
        '分析',
        '观感',
        '解析',
        '隐藏细节',
        '放大镜',
        '动画制作',
        '脑筋急转弯'
      ];
      if (BAD_TOKEN.some((k) => oneLine.includes(k))) return true;

      return false;
    }

    const __guaxiElementIds = new WeakMap();
    let __guaxiElementSeq = 0;

    function getElementId(el) {
      if (!el) return '';
      const existing = __guaxiElementIds.get(el);
      if (existing) return existing;
      const id = `${PROVIDER}-el-${++__guaxiElementSeq}`;
      __guaxiElementIds.set(el, id);
      return id;
    }

    function pickMessageContainer(el) {
      if (!el || !el.closest) return el;
      return (
        el.closest('[data-message-id]') ||
        el.closest('[data-testid*="message" i]') ||
        el.closest('[role="article" i]') ||
        el.closest('article') ||
        el.closest('[class*="message" i]') ||
        el.closest('[class*="bubble" i]') ||
        el.closest('[class*="chat" i]') ||
        el
      );
    }

    function collectGenericMessages() {
      const input = findInput();
      const root =
        (input && input.closest('main, [role="main"], body')) ||
        document.querySelector('main, [role="main"], body') ||
        document.body;

      let blocks = Array.from(root.querySelectorAll('p, li'));
      if (blocks.length < 2) {
        blocks = Array.from(root.querySelectorAll('div'));
      }

      const out = [];
      const seenContainers = new Set();

      for (let i = 0; i < blocks.length; i++) {
        const b = blocks[i];

        if (
          b.closest('nav, header, footer, aside, form, button, [role="navigation"], [role="toolbar"], [role="menu"], [role="dialog"], [aria-label*="工具"], [aria-label*="Tool"]')
        ) {
          continue;
        }

        if (input && (b === input || b.contains?.(input) || input.contains?.(b) || b.closest('textarea, [contenteditable="true"], [role="textbox"]'))) {
          continue;
        }

        const container = pickMessageContainer(b);
        if (!container) continue;
        if (seenContainers.has(container)) continue;
        seenContainers.add(container);

        let content = (PROVIDER === 'yuanbao') ? markdownOf(container) : textOf(container);
        if (!content) continue;
        if (content.length < 10) continue;
        // 自己刚发出去的问题不能当成模型回答回传。
        const lastPrompt = (window.__GUAXI_LAST_PROMPT__ || '').toString();
        if (lastPrompt && (content === lastPrompt || (content.length > 80 && lastPrompt.startsWith(content.slice(0, 80))))) {
          continue;
        }
        if (content.length > 12000) content = content.slice(-12000);

        if (PROVIDER === 'yuanbao' && looksLikeYuanbaoUi(content)) continue;

        if (/^([\u4e00-\u9fa5A-Za-z0-9]+\s+){1,8}[\u4e00-\u9fa5A-Za-z0-9]+$/.test(content) && content.length < 60) {
          continue;
        }

        const attrId = (container.getAttribute && (container.getAttribute('data-message-id') || container.getAttribute('data-id'))) || '';
        const message_id = (attrId || container.id || getElementId(container)).toString();
        if (!message_id) continue;

        out.push({ role: 'assistant', content, message_id });
      }

      return out.slice(-12);
    }

    // 千问（qianwen.com）的结构固定：每个 .chat-round 里问题在 wrapper-question、
    // 回答在 wrapper-answer，正文是 .qk-markdown。只看最后一轮，避免把历史回答当成本次结果。
    function collectQianwenMessages() {
      const rounds = Array.from(document.querySelectorAll('.chat-round'));
      const round = rounds[rounds.length - 1];
      if (!round) return [];
      // 刚发出去的问题还没形成新一轮时，最后一轮仍是旧问答：不能把它当成这次的回答。
      const lastPrompt = String(window.__GUAXI_LAST_PROMPT__ || '').replace(/\s+/g, '');
      if (lastPrompt) {
        const question = textOf(round.querySelector('[class*="wrapper-question"]')).replace(/\s+/g, '');
        if (!question.includes(lastPrompt.slice(0, 16))) return [];
      }
      const answer =
        round.querySelector('[class*="wrapper-answer"] .qk-markdown') ||
        round.querySelector('[class*="message-select-wrapper-answer"] .qk-markdown') ||
        round.querySelector('[class*="wrapper-answer"]');
      if (!answer) return [];
      const content = textOf(answer);
      if (!content || content.length < 2) return [];
      return [{ role: 'assistant', content: content.slice(0, 12000), message_id: `a-${getElementId(answer)}` }];
    }

    function collectMessages() {
      if (PROVIDER === 'chatgpt') return collectChatgptMessages();
      if (PROVIDER === 'qwen') return collectQianwenMessages();
      return collectGenericMessages();
    }

    let lastSessionId = '';
    let lastSentHashById = new Map();
    let timer = null;

    async function push() {
      try {
        const session_id = getSessionId();
        if (!session_id) return;

        if (session_id !== lastSessionId) {
          lastSessionId = session_id;
          lastSentHashById = new Map();
        }

        const messages = collectMessages();
        if (!messages.length) return;

        const delta = [];
        for (const m of messages) {
          const id = (m && m.message_id) ? String(m.message_id) : '';
          if (!id) continue;
          const sig = hashString(String(m.content || ''));
          const prev = lastSentHashById.get(id);
          if (prev === sig) continue;
          delta.push(m);
        }

        if (!delta.length) return;

        for (const m of delta) {
          const id = String(m.message_id || '');
          if (!id) continue;
          lastSentHashById.set(id, hashString(String(m.content || '')));
        }

        await post({
          provider: PROVIDER,
          model_id: window.__GUAXI_MODEL_ID__,
          topic_id: window.__GUAXI_TOPIC_ID__,
          session_id,
          messages: delta
        });
      } catch (e) { }
    }

    const observer = new MutationObserver(() => {
      if (timer) return;
      const wait = PROVIDER === 'yuanbao' ? 250 : 800;
      timer = setTimeout(() => {
        timer = null;
        pushStatus();
        push();
      }, wait);
    });

    observer.observe(document.documentElement, { childList: true, subtree: true, characterData: true });

    setInterval(() => {
      pushStatus();
      push();
    }, PROVIDER === 'yuanbao' ? 1500 : 3500);

    pushStatus();
    push();
  } catch (e) { }
})();