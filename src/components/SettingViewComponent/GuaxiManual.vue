<template>
  <div class="manual">
    <n-card title="用户手册" size="small">
      <n-space vertical size="large" style="width: 100%">
        <n-alert type="info" :show-icon="true">
          本页为 Guaxi 用户手册。需要配图时，请将截图放入 <n-tag size="small">src/assets/manual</n-tag>，
          并把对应“占位图”替换为真实图片。
        </n-alert>

        <div class="toc">
          <div class="toc-title">目录</div>
          <ul class="toc-list">
            <li><a href="#intro">软件简介</a></li>
            <li><a href="#features">主要功能</a></li>
            <li><a href="#quickstart">快速开始</a></li>
            <li><a href="#pages">页面与功能详解</a></li>
            <li><a href="#setting">设置：大模型与 Markdown 转换</a></li>
            <li><a href="#faq">常见问题</a></li>
          </ul>
        </div>

        <n-divider />

        <section id="intro">
          <h2 class="h2">软件简介</h2>
          <p class="p">
            Guaxi 是一款面向“阅读-整理-检索-对话-知识库”的桌面应用。你可以导入/管理本地资料，阅读并生成笔记，
            配合大模型进行对话与知识沉淀，并通过知识库能力进行进一步整理。
          </p>

          <div class="figure">
            <div class="figure-box">
              <div class="figure-title">占位图：软件主界面</div>
              <div class="figure-sub">建议文件：src/assets/manual/overview.png</div>
            </div>
            <div class="figure-caption">图 1：Guaxi 总览（占位）</div>
          </div>
        </section>

        <n-divider />

        <section id="features">
          <h2 class="h2">主要功能</h2>

          <div class="feature-grid">
            <div class="feature-card">
              <div class="feature-title">资料导入与管理</div>
              <div class="feature-desc">建立文件库根目录，集中管理导入资料，支持搜索定位。</div>
            </div>
            <div class="feature-card">
              <div class="feature-title">阅读与摘录</div>
              <div class="feature-desc">内置阅读器能力（Markdown/文本/Word 等），支持导航、检索与辅助操作。</div>
            </div>
            <div class="feature-card">
              <div class="feature-title">聊天与模型选择</div>
              <div class="feature-desc">配置多家模型 Key，进行多会话对话；可切换模型、管理话题与消息。</div>
            </div>
            <div class="feature-card">
              <div class="feature-title">知识库管理</div>
              <div class="feature-desc">设置知识库根目录，创建/管理知识库；支持 Git 远程仓库配置。</div>
            </div>
            <div class="feature-card">
              <div class="feature-title">工具与应用</div>
              <div class="feature-desc">应用中心可打开不同工具型应用窗口（示例：翻译等）。</div>
            </div>
            <div class="feature-card">
              <div class="feature-title">全局设置</div>
              <div class="feature-desc">统一管理笔记库/资源库、服务器与大模型等配置。</div>
            </div>
          </div>
        </section>

        <n-divider />

        <section id="quickstart">
          <h2 class="h2">快速开始</h2>

          <h3 class="h3">1）首次启动建议完成的配置</h3>
          <ol class="ol">
            <li>
              打开 <n-tag size="small">设置</n-tag> → <n-tag size="small">笔记库</n-tag>：
              配置 <b>文件库根目录</b>、<b>资源库根目录</b>（截图/图片等）、<b>知识库根目录</b>（可不配，默认程序同级目录下 KnowledgeBases）。
            </li>
            <li>
              打开 <n-tag size="small">设置</n-tag> → <n-tag size="small">大模型</n-tag>：
              填入需要使用的模型 API Key，并点击保存。
            </li>
            <li>
              若需要 Markdown 转换能力（模型/运行环境），在 <n-tag size="small">设置</n-tag> → <n-tag size="small">大模型</n-tag> 中安装：
              <b>Markdown 转换模型</b>（解压到 utils/Models/surya_models）与 <b>模型运行环境</b>（拷贝到 utils/MarkdownConvertor）。
            </li>
          </ol>

          <div class="figure">
            <div class="figure-box">
              <div class="figure-title">占位图：设置-笔记库与资源库</div>
              <div class="figure-sub">建议文件：src/assets/manual/setting-noterepo.png</div>
            </div>
            <div class="figure-caption">图 2：建议的首次配置（占位）</div>
          </div>

          <h3 class="h3">2）基本使用流程（建议）</h3>
          <ol class="ol">
            <li>导入资料到文件库 → 在“资料/仓库”页面管理与搜索。</li>
            <li>打开文件进行阅读 → 在阅读器内进行导航与整理。</li>
            <li>在“聊天”页面发起对话 → 结合阅读内容进行问答与总结。</li>
            <li>在“知识库”页面创建/维护知识库 → 将整理后的内容沉淀与版本管理。</li>
          </ol>
        </section>

        <n-divider />

        <section id="pages">
          <h2 class="h2">页面与功能详解</h2>

          <div class="page-section">
            <div class="page-title">首页（Home）</div>
            <div class="page-desc">
              首页包含顶部工具条与主内容区。顶部提供侧栏展开/收起、搜索、截图、打开其他页面入口等能力（以实际按钮为准）。
              左侧通常为文件树/目录，右侧为辅助面板（如大纲/信息面板），均支持拖拽调整宽度。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：首页布局</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-home.png</div>
              </div>
              <div class="figure-caption">图 3：首页（占位）</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">资料库（Repository）</div>
            <div class="page-desc">
              该页面用于管理导入的文件资料。顶部提供“搜索导入文件”的输入框，支持关键词搜索与下拉选项；
              右上角有截图等快捷入口；左右侧栏支持折叠与拖拽调整宽度。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：资料库搜索</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-repo-search.png</div>
              </div>
              <div class="figure-caption">图 4：资料库搜索（占位）</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">搜索（Search）</div>
            <div class="page-desc">
              搜索页面采用三栏布局：左侧分类/列表，中间列表视图，右侧详情阅读面板。左右栏可折叠与拖拽调整宽度；
              选择条目后会在右侧加载详情。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：搜索页面三栏布局</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-search.png</div>
              </div>
              <div class="figure-caption">图 5：搜索页面（占位）</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">聊天（Chat）</div>
            <div class="page-desc">
              聊天页面用于与大模型对话。顶部包含模型选择器与搜索占位输入框，并提供打开 Web 模型、侧栏折叠等入口。
              中间区域为消息流与输入区，通常支持多会话/多话题管理（以当前界面为准）。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：聊天页面与模型选择</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-chat.png</div>
              </div>
              <div class="figure-caption">图 6：聊天（占位）</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">知识库（Brain）</div>
            <div class="page-desc">
              知识库页面用于创建与管理知识库内容，并提供“创建知识库”表单入口。建议先在设置中配置知识库根目录，
              再在此页面创建新的知识库结构。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：知识库管理与创建</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-brain.png</div>
              </div>
              <div class="figure-caption">图 7：知识库（占位）</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">应用（Application）</div>
            <div class="page-desc">
              应用页面提供可打开的工具/应用集合：左侧为分类，中心为应用卡片列表；点击卡片会打开独立窗口（通过后端命令）。
              示例应用包含“语言翻译”等，部分应用可能仍为占位实现。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：应用中心</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-application.png</div>
              </div>
              <div class="figure-caption">图 8：应用中心（占位）</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">设置（Setting）</div>
            <div class="page-desc">
              设置页面为三栏布局：左侧为设置菜单，中间为设置内容区，右侧为扩展面板。左侧菜单包含：
              笔记库、外观、服务器、大模型、关于，以及本页“用户手册”。
            </div>
            <div class="figure">
              <div class="figure-box">
                <div class="figure-title">占位图：设置页面与菜单</div>
                <div class="figure-sub">建议文件：src/assets/manual/page-setting.png</div>
              </div>
              <div class="figure-caption">图 9：设置页面（占位）</div>
            </div>
          </div>
        </section>

        <n-divider />

        <section id="setting">
          <h2 class="h2">设置：大模型与 Markdown 转换</h2>

          <h3 class="h3">大模型 API Key</h3>
          <p class="p">
            进入 <n-tag size="small">设置</n-tag> → <n-tag size="small">大模型</n-tag>，可分别填写：
            ChatGPT / DeepSeek / Qwen / Kimi / 豆包 的 API Key。填写完成后点击“保存”。
          </p>

          <h3 class="h3">ChatRecord 保存路径</h3>
          <p class="p">
            同页面提供 ChatRecord 保存路径配置。未配置时默认使用软件同级目录下的 <b>ChatRecord</b>。
          </p>

          <h3 class="h3">Markdown 转换模型</h3>
          <p class="p">
            选择模型压缩包（zip）后点击“安装”，程序会将模型解压到软件同级目录下：
            <b>utils/Models/surya_models</b>。如果压缩包内已包含 <b>surya_models</b> 目录，则会按目录结构放置到 <b>utils/Models</b> 下。
          </p>

          <h3 class="h3">模型运行环境</h3>
          <p class="p">
            选择运行环境压缩包后点击“拷贝”，程序会将压缩包拷贝到软件同级目录下：
            <b>utils/MarkdownConvertor</b>。
          </p>

          <div class="figure">
            <div class="figure-box">
              <div class="figure-title">占位图：设置-大模型与转换模型</div>
              <div class="figure-sub">建议文件：src/assets/manual/setting-llm.png</div>
            </div>
            <div class="figure-caption">图 10：大模型与转换配置（占位）</div>
          </div>
        </section>

        <n-divider />

        <section id="faq">
          <h2 class="h2">常见问题</h2>

          <div class="qa">
            <div class="q">Q：为什么“用户手册”菜单点了没有内容？</div>
            <div class="a">
              A：请确认已应用“菜单新增”和“内容映射”的修改；并确保当前选中菜单 key 为
              <n-tag size="small">manual</n-tag>。
            </div>
          </div>

          <div class="qa">
            <div class="q">Q：Markdown 转换模型安装后仍不可用？</div>
            <div class="a">
              A：检查软件同级目录下是否存在 <b>utils/Models/surya_models</b>；
              若模型/环境更新后仍异常，建议重启应用并确认压缩包内容结构符合预期。
            </div>
          </div>

          <div class="qa">
            <div class="q">Q：知识库的 Git 推送/拉取使用什么凭据？</div>
            <div class="a">
              A：该配置说明为“直接调用系统 git 命令”，通常使用你本机 Git 的配置与凭据环境。
              如遇权限问题，请先在系统终端中确认同一远程仓库可正常 git 操作。
            </div>
          </div>
        </section>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { NCard, NSpace, NAlert, NDivider, NTag } from 'naive-ui'
</script>

<style scoped>
.manual {
  width: 100%;
  box-sizing: border-box;
}

.h2 {
  margin: 0;
  font-size: 18px;
  line-height: 1.4;
}

.h3 {
  margin: 12px 0 6px;
  font-size: 15px;
  line-height: 1.4;
}

.p {
  margin: 8px 0 0;
  color: var(--guaxi-text-primary);
  line-height: 1.8;
}

.toc {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 12px;
  background: var(--guaxi-panel-bg);
}

.toc-title {
  font-weight: 600;
  margin-bottom: 8px;
}

.toc-list {
  margin: 0;
  padding-left: 18px;
}

.toc-list a {
  color: var(--n-color-primary);
  text-decoration: none;
}

.toc-list a:hover {
  text-decoration: underline;
}

.figure {
  margin-top: 12px;
}

.figure-box {
  border: 1px dashed var(--guaxi-border);
  border-radius: 12px;
  height: 180px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  background: rgba(127, 127, 127, 0.08);
  user-select: none;
}

.figure-title {
  font-weight: 600;
}

.figure-sub {
  margin-top: 6px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.figure-caption {
  margin-top: 6px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.feature-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 10px;
}

.feature-card {
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 12px;
  background: var(--guaxi-panel-bg);
}

.feature-title {
  font-weight: 600;
  margin-bottom: 6px;
}

.feature-desc {
  color: var(--guaxi-text-secondary);
  line-height: 1.6;
}

.page-section {
  padding: 10px 0;
}

.page-title {
  font-weight: 700;
  margin-bottom: 6px;
}

.page-desc {
  color: var(--guaxi-text-primary);
  line-height: 1.8;
}

.ol {
  margin: 8px 0 0;
  padding-left: 18px;
  line-height: 1.9;
}

.qa {
  padding: 10px 0;
}

.q {
  font-weight: 700;
  margin-bottom: 6px;
}

.a {
  color: var(--guaxi-text-primary);
  line-height: 1.8;
}
</style>