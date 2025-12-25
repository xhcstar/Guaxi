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

      try {
        const style = window.getComputedStyle(input);
        if (style && (style.display === 'none' || style.visibility === 'hidden')) return false;
      } catch (e) { }

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

    function collectGenericMessages() {
      const BAD = [
        '深度思考',
        '联网搜索',
        '工具',
        '下载电脑版',
        '内容由AI生成',
        '文件拖动到此处即可上传',
        '支持文件格式'
      ];

      const input = findInput();
      const root =
        (input && input.closest('main, [role="main"], body')) ||
        document.querySelector('main, [role="main"], body') ||
        document.body;

      const blocks = Array.from(root.querySelectorAll('p, li, div'));
      const out = [];
      const seen = new Set();

      for (let i = 0; i < blocks.length; i++) {
        const b = blocks[i];

        if (
          b.closest('nav, header, footer, aside, form, button, [role="navigation"], [role="toolbar"], [role="menu"], [role="dialog"], [aria-label*="工具"], [aria-label*="Tool"]')
        ) {
          continue;
        }

        const content = textOf(b);
        if (!content) continue;
        if (content.length < 10) continue;
        if (content.length > 2000) continue;

        if (BAD.some((k) => content.includes(k)) && content.length < 80) continue;

        if (/^([\u4e00-\u9fa5A-Za-z0-9]+\s+){1,8}[\u4e00-\u9fa5A-Za-z0-9]+$/.test(content) && content.length < 60) {
          continue;
        }

        if (seen.has(content)) continue;
        seen.add(content);

        const message_id = `generic-${i}`;
        out.push({ role: 'assistant', content, message_id });
      }

      return out.slice(-20);
    }

    function collectMessages() {
      if (PROVIDER === 'chatgpt') return collectChatgptMessages();
      return collectGenericMessages();
    }

    let lastSig = '';
    let lastSessionId = '';
    let lastSentLen = 0;
    let timer = null;

    async function push() {
      try {
        const session_id = getSessionId();
        if (!session_id) return;

        if (session_id !== lastSessionId) {
          lastSessionId = session_id;
          lastSentLen = 0;
          lastSig = '';
        }

        const messages = collectMessages();
        if (!messages.length) return;

        if (lastSentLen > messages.length) lastSentLen = 0;

        const sig = JSON.stringify(messages.slice(-6));
        if (sig === lastSig) return;

        const delta = messages.slice(lastSentLen);
        if (!delta.length) return;

        lastSig = sig;
        lastSentLen = messages.length;

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
      timer = setTimeout(() => {
        timer = null;
        pushStatus();
        push();
      }, 800);
    });

    observer.observe(document.documentElement, { childList: true, subtree: true, characterData: true });

    setInterval(() => {
      pushStatus();
      push();
    }, 3500);

    pushStatus();
    push();
  } catch (e) { }
})();