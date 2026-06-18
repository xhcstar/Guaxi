export const MARKDOWN_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'];

export const CODE_BLOCK_SNIPPETS = [
  { label: '纯代码块', key: 'plain', language: '', body: '```\n$1\n```' },
  { label: 'C', key: 'c', language: 'c', body: '```c\n$1\n```' },
  { label: 'C++', key: 'cpp', language: 'cpp', body: '```cpp\n$1\n```' },
  { label: 'Python', key: 'python', language: 'python', body: '```python\n$1\n```' },
  { label: 'Shell', key: 'bash', language: 'bash', body: '```bash\n$1\n```' },
  { label: 'Mermaid', key: 'mermaid', language: 'mermaid', body: '```mermaid\n$1\n```' },
  { label: 'Rust', key: 'rust', language: 'rust', body: '```rust\n$1\n```' },
  { label: 'JavaScript', key: 'javascript', language: 'javascript', body: '```javascript\n$1\n```' },
  { label: 'TypeScript', key: 'typescript', language: 'typescript', body: '```typescript\n$1\n```' },
  { label: 'Lua', key: 'lua', language: 'lua', body: '```lua\n$1\n```' },
  { label: 'HTML', key: 'html', language: 'html', body: '```html\n$1\n```' },
  { label: 'Perl', key: 'perl', language: 'perl', body: '```perl\n$1\n```' },
  { label: 'JSON', key: 'json', language: 'json', body: '```json\n$1\n```' },
  { label: 'XML', key: 'xml', language: 'xml', body: '```xml\n$1\n```' },
  { label: 'TeX', key: 'tex', language: 'tex', body: '```tex\n$1\n```' },
  { label: 'C#', key: 'csharp', language: 'csharp', body: '```csharp\n$1\n```' },
  { label: 'Lisp', key: 'lisp', language: 'lisp', body: '```lisp\n$1\n```' },
  { label: 'TOML', key: 'toml', language: 'toml', body: '```toml\n$1\n```' },
  { label: 'PowerShell', key: 'powershell', language: 'powershell', body: '```powershell\n$1\n```' },
];

export const SHARED_MARKDOWN_SNIPPETS = [
  {
    scope: 'markdown',
    prefix: 'gxtable',
    description: '插入 Markdown 表格',
    body: ['| 列1 | 列2 |', '| --- | --- |', '| 内容1 | 内容2 |'].join('\n'),
  },
  ...CODE_BLOCK_SNIPPETS.map((item) => ({
    scope: 'markdown',
    prefix: item.language ? `gxcode-${item.language}` : 'gxcode',
    description: `插入${item.label}`,
    body: item.body,
  })),
];

export const clampInt = (value, min, max) => {
  const n = Number(value);
  if (!Number.isFinite(n)) return min;
  return Math.max(min, Math.min(max, Math.floor(n)));
};

export const buildMarkdownTable = (rows, cols, options = {}) => {
  const maxRows = Number(options.maxRows || 120);
  const maxCols = Number(options.maxCols || 40);
  const r = clampInt(rows, 2, maxRows);
  const c = clampInt(cols, 1, maxCols);

  const header = Array.from({ length: c }, (_, i) => `列${i + 1}`);
  const sep = Array.from({ length: c }, () => '---');

  const lines = [];
  lines.push(`| ${header.join(' | ')} |`);
  lines.push(`| ${sep.join(' | ')} |`);

  const bodyRows = Math.max(1, r - 1);
  for (let i = 0; i < bodyRows; i += 1) {
    const cells = Array.from({ length: c }, (_, idx) => (i === 0 ? `内容${idx + 1}` : ''));
    lines.push(`| ${cells.join(' | ')} |`);
  }

  return `${lines.join('\n')}\n`;
};

export const sanitizeResourceFileSegment = (raw, fallback = 'image') => {
  const normalized = String(raw || '')
    .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '');
  return normalized || fallback;
};

export const joinResourceRepoPath = (...parts) => parts
  .map((part, index) => {
    const normalized = String(part || '').replace(/\\/g, '/');
    return index === 0 ? normalized.replace(/\/+$/g, '') : normalized.replace(/^\/+|\/+$/g, '');
  })
  .filter(Boolean)
  .join('/');

export const normalizePickedLocalPath = (path) => {
  const normalized = String(path || '').trim().replace(/\\/g, '/');
  return normalized.replace(/^\/([a-zA-Z]:\/)/, '$1');
};

export const getFileNameFromPath = (path) => {
  const normalized = String(path || '').replace(/\\/g, '/');
  return normalized.split('/').filter(Boolean).pop() || '';
};

export const getMarkdownImageExtension = (path) => {
  const fileName = getFileNameFromPath(path);
  const ext = String(fileName.split('.').pop() || '').trim().toLowerCase();
  return MARKDOWN_IMAGE_EXTENSIONS.includes(ext) ? ext : 'png';
};

export const buildMarkdownImageResourceTarget = (sourcePath, date = new Date()) => {
  const year = String(date.getFullYear());
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const stamp = [
    year,
    month,
    day,
    '_',
    String(date.getHours()).padStart(2, '0'),
    String(date.getMinutes()).padStart(2, '0'),
    String(date.getSeconds()).padStart(2, '0'),
    '_',
    String(date.getMilliseconds()).padStart(3, '0'),
  ].join('');
  const sourceName = getFileNameFromPath(sourcePath);
  const ext = getMarkdownImageExtension(sourceName);
  const stem = sanitizeResourceFileSegment(sourceName.replace(/\.[^.]+$/, ''), 'image');
  const fileName = `${stem}-${stamp}.${ext}`;
  const relativeDir = joinResourceRepoPath('images', year, month, day);
  const relativePath = joinResourceRepoPath(relativeDir, fileName);

  return {
    fileName,
    relativeDir,
    relativePath,
    alt: stem,
  };
};

export const buildMarkdownImageSyntax = ({ alt = 'image', url = '' } = {}) => {
  return `![${String(alt || 'image')}](${String(url || '')})\n`;
};

export const getSharedEditingCommandManifest = () => ({
  version: 1,
  generatedBy: 'Guaxi',
  markdownImageExtensions: MARKDOWN_IMAGE_EXTENSIONS,
  snippets: SHARED_MARKDOWN_SNIPPETS,
  commands: [
    { name: 'insert_table', description: '插入 Markdown 表格', args: ['rows', 'cols'] },
    { name: 'import_image', description: '导入图片到 Guaxi 资源库并插入 Markdown 图片语法' },
    { name: 'edit_code_block', description: '将 Markdown 代码块映射到临时语言文件交给 Neovim LSP 编辑' },
  ],
});

