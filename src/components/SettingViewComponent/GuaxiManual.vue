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
            <li><a href="#editor">编辑器与右侧面板（重点）</a></li>
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
              <img class="figure-img" src="../../assets/manual/overview.png" alt="软件主界面" />
            </div>
            <div class="figure-caption">图 1：Guaxi 总览</div>
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
              <img class="figure-img" src="../../assets/manual/setting-noterepo.png" alt="设置-笔记库与资源库" />
            </div>
            <div class="figure-caption">图 2：建议的首次配置</div>
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

          <div id="editor" class="page-section">
            <div class="page-title">编辑器/阅读器（重点）</div>
            <div class="page-desc">
              首页（Home）为主要编辑入口，内置 Monaco 编辑器（支持 Markdown 常用操作、右键菜单与表格辅助）。资料库（Repository）右键“打开”后会启动阅读器窗口（Markdown/Text/Word/Epub 等），提供顶部工具栏、页内搜索、书签与右侧“我的笔记”面板。
            </div>

            <div class="manual-subtitle">Home：编辑器快捷键速查（Monaco）</div>
            <ul class="manual-ul">
              <li>保存：保存当前文件内容（操作：编辑后按快捷键；快捷键：Ctrl+S）。</li>
              <li>模式：编辑 / 预览 / 分栏 / 实时预览（操作：点击编辑器模式切换按钮；快捷键：Ctrl+1 / Ctrl+2 / Ctrl+3 / Ctrl+4）。</li>
              <li>加粗：为选中内容添加/取消 **（操作：选中文本后点击工具条“加粗”；快捷键：Ctrl+B）。</li>
              <li>斜体：为选中内容添加/取消 *（操作：选中文本后点击工具条“斜体”；快捷键：Ctrl+I）。</li>
              <li>删除线：为选中内容添加/取消 ~~（操作：选中文本后点击工具条“删除线”；快捷键：Ctrl+Shift+X）。</li>
              <li>插入链接：插入 [文字](URL)（操作：选中文本或放置光标后触发；快捷键：Ctrl+K）。</li>
              <li>插入表格：插入 Markdown 表格骨架（操作：在光标处插入；快捷键：Ctrl+Alt+T）。</li>
              <li>表格：下方插入行（Ctrl+Alt+↓）；删除当前行（Ctrl+Alt+Shift+↓）。</li>
              <li>表格：右侧插入列（Ctrl+Alt+→）；删除当前列（Ctrl+Alt+Shift+→）。</li>
              <li>编辑位置跳转：返回上次编辑位置（Alt+←）；前进到下一编辑位置（Alt+→）。</li>
              <li>剪切/复制/粘贴：Ctrl+X / Ctrl+C / Ctrl+V（也可用右键菜单）。</li>
            </ul>

            <div class="manual-subtitle">Home：编辑器右键菜单</div>
            <ul class="manual-ul">
              <li>剪切 / 复制 / 粘贴：在编辑器内右键 → 选择对应操作（快捷键：Ctrl+X / Ctrl+C / Ctrl+V）。</li>
              <li>Markdown 格式化（全文）：右键 → Markdown 格式化（全文），仅 Markdown 文件可用（快捷键：无）。</li>
              <li>Markdown 格式化（选中）：先选中文本 → 右键 → Markdown 格式化（选中）（快捷键：无）。</li>
              <li>跳转到上次编辑位置：右键 → 跳转到上次编辑位置（快捷键：Alt+←）。</li>
              <li>跳转到下一编辑位置：右键 → 跳转到下一编辑位置（快捷键：Alt+→）。</li>
              <li>插入Markdown模板：右键 → 插入Markdown模板，会在光标/选区插入示例模板（快捷键：无）。</li>
              <li>命令面板：右键 → 命令面板（快捷键：Monaco 默认 F1）。</li>
            </ul>

            <div class="manual-subtitle">Home：选中浮动工具条</div>
            <ul class="manual-ul">
              <li>触发：在编辑器内框选文字后松开鼠标，会弹出浮动工具条。</li>
              <li>常用项：加粗（Ctrl+B）、斜体（Ctrl+I）、删除线（Ctrl+Shift+X）、插入链接（Ctrl+K）、内联代码、插入表格（Ctrl+Alt+T）。</li>
            </ul>

            <div class="manual-subtitle">Home：表格浮动工具条</div>
            <ul class="manual-ul">
              <li>触发：光标位于 Markdown 表格内时显示，用于快速增删行/列。</li>
              <li>下方插入行 / 删除当前行：Ctrl+Alt+↓ / Ctrl+Alt+Shift+↓。</li>
              <li>右侧插入列 / 删除当前列：Ctrl+Alt+→ / Ctrl+Alt+Shift+→。</li>
            </ul>

            <div class="manual-subtitle">阅读器：工具栏图标与作用（Repository 右键“打开”后）</div>
            <ul class="manual-ul">
              <li>← / →：上一页 / 下一页（操作：点击顶部按钮；快捷键：PageUp / PageDown）。</li>
              <li>适合宽度：将页面缩放到合适宽度（操作：点击“适合宽度”；快捷键：Ctrl+0）。</li>
              <li>－ / ＋：缩小 / 放大（操作：点击缩放按钮；快捷键：Ctrl+- / Ctrl+=）。</li>
              <li>布局（切换布局）：左右分栏 / 仅左侧 / 仅右侧 / 大纲（操作：点击“切换布局”下拉；快捷键：Ctrl+1 / Ctrl+2 / Ctrl+3 / Ctrl+4）。</li>
              <li>书签：添加/取消当前页书签；书签列表：打开书签库并跳转到指定页（快捷键：无）。</li>
              <li>笔记：显示/隐藏右侧“我的笔记”面板（操作：点击“笔记”；快捷键：Ctrl+N）。</li>
              <li>译文：显示/隐藏译文（操作：点击“译文”；快捷键：Ctrl+T）。</li>
            </ul>

            <div class="manual-subtitle">阅读器：模式切换与分栏（重点）</div>
            <ul class="manual-ul">
              <li>左右分栏：左侧显示原文/预览，右侧显示渲染后的内容（最适合对照阅读与做笔记）。</li>
              <li>仅左侧 / 仅右侧：专注单栏阅读；中间拖拽线仅在“左右分栏”模式出现。</li>
              <li>大纲：切换显示大纲面板，便于按标题快速定位。</li>
            </ul>

            <div class="manual-subtitle">阅读器：页内搜索（重点）</div>
            <ul class="manual-ul">
              <li>打开：Ctrl+F；关闭：Esc 或搜索栏“关闭”按钮。</li>
              <li>跳转：Enter 下一个、Shift+Enter 上一个；也可点击搜索栏中的 ← / →。</li>
              <li>计数：显示为 当前/总数，例如 3/12。</li>
            </ul>

            <div class="manual-subtitle">阅读器：右侧“我的笔记”面板（重点）</div>
            <ul class="manual-ul">
              <li>打开/关闭：工具栏“笔记”按钮或 Ctrl+N；面板右上角“×”关闭。</li>
              <li>新增卡片：点击“添加新卡片”。</li>
              <li>编辑：点击卡片正文进入编辑；保存（保存图标，支持 Ctrl+S）；退出编辑（退出图标）。</li>
              <li>删除：垃圾桶图标删除当前卡片。</li>
            </ul>
          </div>

          <div class="page-section">
            <div class="page-title">各页面顶部栏图标速查</div>
            <div class="page-desc">
              各页面顶部栏的图标会随页面而变化，下列为常见入口及其作用。
            </div>
            <ul class="manual-ul">
              <li>左侧栏开关（面板收起图标）：切换左侧边栏（文件树/导航/分类）。</li>
              <li>右侧栏开关（面板收起图标）：切换右侧边栏（详情/大纲/筛选/信息面板）。</li>
              <li>放大镜（首页/资料库）：用于搜索文件或内容；资料库支持更细的搜索类型下拉选项。</li>
              <li>截图：首页为“截图并插入”；资料库为“截图保存到当前目录 images 文件夹”。</li>
              <li>首页额外入口：新建知识库 / 打开大纲 / 模板管理 / 纵向分屏 / 横向分屏。</li>
              <li>聊天额外入口：打开网页模型（仅当当前模型为网页模式时可用）。</li>
            </ul>
          </div>

          <div class="page-section">
            <div class="page-title">首页（Home）</div>
            <div class="page-desc">
              首页包含顶部工具条与主内容区。顶部提供侧栏展开/收起、搜索、截图并插入、模板管理、分屏等能力；左侧通常为文件树/目录，右侧为辅助面板（如大纲/搜索导航），左右栏均支持拖拽调整宽度。
            </div>

            <div class="manual-subtitle">顶部栏图标与使用</div>
            <ul class="manual-ul">
              <li>左侧栏开关（面板收起图标）：展开/收起左侧边栏（操作：点击左上角图标；快捷键：无）。</li>
              <li>搜索（放大镜）：搜索文件/内容/摘录（操作：点击输入框 → 选择类型 → 输入关键词 → 回车；快捷键：Enter 执行）。</li>
              <li>新建知识库（Book+）：创建新的知识库（操作：点击图标 → 在弹窗中填写信息并提交；快捷键：无）。</li>
              <li>大纲（列表图标）：打开/切换右侧大纲面板（操作：点击图标；快捷键：无）。</li>
              <li>模板管理（Box 图标）：打开模板管理器窗口（操作：点击图标 → 在模板列表中点“插入”把模板发送到编辑器；快捷键：无）。</li>
              <li>截图并插入（Screenshot 图标）：截图并将识别结果以 Markdown 插入到当前编辑器（操作：点击图标 → 按系统提示完成截图；快捷键：无）。</li>
              <li>纵向分屏：将编辑区分为左右两个编辑器（操作：点击“纵向分屏”；快捷键：无）。</li>
              <li>横向分屏：将编辑区分为上下两个编辑器（操作：点击“横向分屏”；快捷键：无）。</li>
              <li>右侧栏开关（面板收起图标）：展开/收起右侧边栏（操作：点击右上角图标；快捷键：无）。</li>
            </ul>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" src="../../assets/manual/page-home.png" alt="首页布局" />
              </div>
              <div class="figure-caption">图 3：首页</div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">资料库（Repository）</div>
            <div class="page-desc">
              该页面用于管理导入的本地资料文件。你可以导入文件并补充元数据（文件库/知识库/标签/描述），通过顶部搜索快速定位；在表格中右键条目可打开阅读器窗口进行阅读与做笔记。
            </div>

            <div class="manual-subtitle">导入文件（重点）</div>
            <ul class="manual-ul">
              <li>入口：点击表格工具栏“导入文件”（Folder+ 图标）。</li>
              <li>选择文件：在“文件路径”行点击“选择文件”，选择本地文件后会自动填充文件名/类型/大小/入库时间等只读字段。</li>
              <li>补充信息：填写“文件描述”，选择或输入“文件库 / 知识库”，填写“文件标签”（多个用英文逗号分隔）。</li>
              <li>提交：点击“提交”完成导入（快捷键：无）。</li>
            </ul>

            <div class="manual-subtitle">顶部栏图标与搜索</div>
            <ul class="manual-ul">
              <li>左侧栏开关：展开/收起左侧边栏（快捷键：无）。</li>
              <li>搜索（放大镜）：点击输入框后选择“按文件名/描述/标签/文件库/知识库/类型”搜索，输入关键词后回车执行。</li>
              <li>高级输入（可选）：在关键词中可使用 <b>lib:</b>、<b>tag:</b>、<b>type:</b>、<b>repo:</b> 组合过滤（例如：<b>lib:Books tag:AI type:pdf</b>）。</li>
              <li>截图（Screenshot 图标）：将截图保存到当前文件同目录下的 <b>images</b> 文件夹（文件名同源文件名，后缀 .png）（快捷键：无）。</li>
              <li>右侧栏开关：展开/收起右侧边栏（快捷键：无）。</li>
            </ul>

            <div class="manual-subtitle">资料条目右键菜单</div>
            <ul class="manual-ul">
              <li>打开：右键某行 → 打开，会创建独立阅读器窗口（支持：pdf、markdown/md、html/htm、epub、word/doc/docx、txt；快捷键：无）。</li>
              <li>打开导入文件所在目录 / 打开笔记文件所在目录：用于在系统文件管理器中定位文件（快捷键：无）。</li>
              <li>编辑：修改条目元数据（描述/标签等）并保存（快捷键：无）。</li>
              <li>删除 / 删除该条目及相关文件：删除当前条目（后者会清理相关文件）（快捷键：无）。</li>
              <li>移动 / 导出 / 分享：对条目执行对应操作（以当前界面为准；快捷键：无）。</li>
            </ul>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" src="../../assets/manual/page-repo-search.png" alt="资料库搜索" />
              </div>
              <div class="figure-caption">图 4：资料库搜索</div>
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
                <img class="figure-img" src="../../assets/manual/page-chat.png" alt="聊天页面与模型选择" />
              </div>
              <div class="figure-caption">图 6：聊天</div>
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
                <img class="figure-img" src="../../assets/manual/page-brain.png" alt="知识库管理与创建" />
              </div>
              <div class="figure-caption">图 7：知识库</div>
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
                <img class="figure-img" src="../../assets/manual/page-application.png" alt="应用中心" />
              </div>
              <div class="figure-caption">图 8：应用中心</div>
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
                <img class="figure-img" src="../../assets/manual/page-setting.png" alt="设置页面与菜单" />
              </div>
              <div class="figure-caption">图 9：设置页面</div>
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
  min-height: 360px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: stretch;
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

.figure-img {
  width: 100%;
  height: auto;
  max-height: 70vh;
  object-fit: contain;
  border-radius: 8px;
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

.manual-subtitle {
  margin-top: 10px;
  font-weight: 700;
}

.manual-ul {
  margin: 8px 0 0;
  padding-left: 18px;
  line-height: 1.9;
}

.manual-ul li {
  margin: 2px 0;
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