<template>
  <div class="manual">
    <n-card title="用户手册" size="small">
      <n-space vertical size="large" style="width: 100%">
        <div class="toc">
          <div class="toc-title">目录</div>
          <ul class="toc-list">
            <li><button type="button" class="toc-link" @click="scrollToSection('intro')">软件简介</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('features')">主要功能</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('quickstart')">快速开始</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('pages')">页面与功能详解</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('editor')">编辑器与右侧面板（重点）</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('setting')">设置：大模型与 Markdown 转换</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('faq')">常见问题</button></li>
          </ul>
        </div>

        <n-divider />

        <section id="intro">
          <h2 class="h2">软件简介</h2>
          <p class="p">
            Guaxi 是一款面向“阅读-整理-检索-对话-知识库”的桌面应用。你可以导入/管理本地资料，阅读并生成笔记，
            配合大模型进行对话与知识沉淀，并通过知识库能力进行进一步整理。资料库用于承载原始文件与元数据，知识库用于沉淀整理结果与版本管理，
            二者协同形成完整的知识闭环。
          </p>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('overview')" alt="软件主界面" />
            </div>
            <div class="figure-caption">图 1：Guaxi 总览</div>
          </div>
        <p class="p">
          本手册按“页面与流程”组织说明，建议先熟悉首页与资料库，再进入聊天与知识库页面完成沉淀与复盘。
        </p>
      </section>

        <n-divider />

        <section id="features">
          <h2 class="h2">主要功能</h2>

          <div class="feature-grid">
            <button type="button" class="feature-card" @click="scrollToSection('repository')">
              <div class="feature-title">资料导入与管理</div>
              <div class="feature-desc">建立文件库根目录，集中管理导入资料，支持搜索定位。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('editor')">
              <div class="feature-title">阅读与摘录</div>
              <div class="feature-desc">内置阅读器能力（Markdown/文本/Word 等），支持导航、检索与辅助操作。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('chat')">
              <div class="feature-title">聊天与模型选择</div>
              <div class="feature-desc">配置多家模型 Key，进行多会话对话；可切换模型、管理话题与消息。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('brain')">
              <div class="feature-title">知识库管理</div>
              <div class="feature-desc">设置知识库根目录，创建/管理知识库；支持 Git 远程仓库配置。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('application')">
              <div class="feature-title">工具与应用</div>
              <div class="feature-desc">应用中心可打开不同工具型应用窗口（示例：翻译等）。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('setting')">
              <div class="feature-title">全局设置</div>
              <div class="feature-desc">统一管理笔记库/资源库、服务器与大模型等配置。</div>
            </button>
          </div>
        </section>

        <n-divider />

        <section id="quickstart">
          <h2 class="h2">快速开始</h2>
          <p class="p">
            首次使用建议完成基础配置与目录规划，便于后续导入、检索与同步操作。
          </p>

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
            <li>
              如需本地模型（LocalDeepseek），在 <n-tag size="small">设置</n-tag> → <n-tag size="small">大模型</n-tag> 中点击“下载/导入”下载或者导入对应的本地大模型。
            </li>
          </ol>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('setting-noterepo')" alt="设置-笔记库与资源库" />
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
          <p class="p">
            本节按页面逐一说明入口、布局与主要功能，并给出对应窗口示意图以便快速对照。
          </p>

          <div id="editor" class="page-section">
            <div class="page-title">编辑器/阅读器（重点）</div>
            <div class="page-desc">
              首页（Home）为主要编辑入口，内置 Monaco 编辑器（支持 Markdown 常用操作、右键菜单与表格辅助）。资料库（Repository）右键“打开”后会启动阅读器窗口（Markdown/Text/Word/Epub 等），提供顶部工具栏、页内搜索、书签与右侧“我的笔记”面板。
              建议在首页完成写作与整理，在阅读器完成资料研读与摘录，二者形成阅读-整理-沉淀的闭环。
            </div>

            <div class="manual-subtitle">Home：编辑器工具条图标（顶部）</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Edit16Filled" /></div>
                <div class="icon-name">Vim 模式</div>
                <div class="icon-desc">启用 Vim 模式（再次点击切换为 <span class="icon-inline" style="margin:0 4px 0 0"><n-icon :component="LogoVimeo" /></span> 图标状态）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SaveOutline" /></div>
                <div class="icon-name">保存</div>
                <div class="icon-desc">保存当前文件（Ctrl+S）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="filePenLineIcon" /></div>
                <div class="icon-name">模式：编辑</div>
                <div class="icon-desc">切换编辑/预览/分屏/实时预览（Ctrl+1/2/3/4）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="eyeIcon" /></div>
                <div class="icon-name">模式：预览</div>
                <div class="icon-desc">同上（预览态图标）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="columns2Icon" /></div>
                <div class="icon-name">模式：分屏预览</div>
                <div class="icon-desc">同上（分屏预览态图标）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="sparklesIcon" /></div>
                <div class="icon-name">模式：实时预览</div>
                <div class="icon-desc">同上（实时预览态图标）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ExitOutline" /></div>
                <div class="icon-name">导出</div>
                <div class="icon-desc">导出 Markdown（PDF / docx / html / tex / epub / odt / txt）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="minusIcon" /></div>
                <div class="icon-name">分隔线</div>
                <div class="icon-desc">插入 Markdown 分隔线。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="undoIcon" /></div>
                <div class="icon-name">撤销</div>
                <div class="icon-desc">撤销编辑操作。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="redoIcon" /></div>
                <div class="icon-name">重做</div>
                <div class="icon-desc">重做编辑操作。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon><CloseOutline /></n-icon></div>
                <div class="icon-name">关闭标签页</div>
                <div class="icon-desc">关闭当前文件标签页。</div>
              </div>
            </div>

            <div class="manual-subtitle">Home：编辑器快捷键速查（Monaco）</div>
            <ul class="manual-ul">
              <li>保存：保存当前文件内容（操作：编辑后按快捷键；快捷键：Ctrl+S）。</li>
              <li>文件内搜索：打开搜索框（操作：编辑器内按快捷键；快捷键：Ctrl+F）。</li>
              <li>模式：编辑 / 预览 / 分栏 / 实时预览（操作：点击编辑器模式切换按钮；快捷键：Ctrl+1 / Ctrl+2 / Ctrl+3 / Ctrl+4）。</li>
              <li>加粗：为选中内容添加/取消 **（操作：选中文本后点击工具条“加粗”；快捷键：Ctrl+B）。</li>
              <li>斜体：为选中内容添加/取消 *（操作：选中文本后点击工具条“斜体”；快捷键：Ctrl+I）。</li>
              <li>删除线：为选中内容添加/取消 ~~（操作：选中文本后点击工具条“删除线”；快捷键：Ctrl+Shift+X）。</li>
              <li>插入链接：插入 [文字](URL)（操作：选中文本或放置光标后触发；快捷键：Ctrl+K）。</li>
              <li>插入表格：插入 Markdown 表格骨架（操作：在光标处插入；快捷键：Ctrl+Alt+T；插入后自动格式化）。</li>
              <li>表格：下方插入行（Ctrl+Alt+↓）；删除当前行（Ctrl+Alt+Shift+↓）。</li>
              <li>表格：右侧插入列（Ctrl+Alt+→）；删除当前列（Ctrl+Alt+Shift+→）。</li>
              <li>表格：单元格移动（Ctrl+Alt+H/J/K/L，分别为左/下/上/右）。</li>
              <li>编辑位置跳转：返回上次编辑位置（Alt+←）；前进到下一编辑位置（Alt+→）。</li>
              <li>剪切/复制/粘贴：Ctrl+X / Ctrl+C / Ctrl+V（也可用右键菜单）。</li>
            </ul>

            <div class="manual-subtitle">Home：编辑器右键菜单</div>
            <ul class="manual-ul">
              <li>剪切 / 复制 / 粘贴：在编辑器内右键 → 选择对应操作（快捷键：Ctrl+X / Ctrl+C / Ctrl+V）。</li>
              <li>文件内搜索：右键 → 文件内搜索（快捷键：Ctrl+F）。</li>
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
            </ul>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="boldIcon" /></div>
                <div class="icon-name">加粗</div>
                <div class="icon-desc">为选中内容添加/取消 **（Ctrl+B）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="italicIcon" /></div>
                <div class="icon-name">斜体</div>
                <div class="icon-desc">为选中内容添加/取消 *（Ctrl+I）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="strikethroughIcon" /></div>
                <div class="icon-name">删除线</div>
                <div class="icon-desc">为选中内容添加/取消 ~~（Ctrl+Shift+X）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="linkIcon" /></div>
                <div class="icon-name">链接</div>
                <div class="icon-desc">插入 [文字](URL)（Ctrl+K）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="codeIcon" /></div>
                <div class="icon-name">行内代码</div>
                <div class="icon-desc">为选中内容包裹反引号。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="quoteIcon" /></div>
                <div class="icon-name">引用</div>
                <div class="icon-desc">插入/取消引用块。</div>
              </div>
            </div>

            <div class="manual-subtitle">Home：表格浮动工具条</div>
            <ul class="manual-ul">
              <li>触发：光标位于 Markdown 表格内时显示，用于快速增删行/列。</li>
            </ul>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="tableRowPlusAfter" /></div>
                <div class="icon-name">新增行</div>
                <div class="icon-desc">在下方插入行（Ctrl+Alt+↓）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="tableRowRemove" /></div>
                <div class="icon-name">删除行</div>
                <div class="icon-desc">删除当前行（Ctrl+Alt+Shift+↓）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="tableColumnPlusAfter" /></div>
                <div class="icon-name">新增列</div>
                <div class="icon-desc">在右侧插入列（Ctrl+Alt+→）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="tableColumnRemove" /></div>
                <div class="icon-name">删除列</div>
                <div class="icon-desc">删除当前列（Ctrl+Alt+Shift+→）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="alignLeftIcon" /></div>
                <div class="icon-name">左对齐</div>
                <div class="icon-desc">设置当前列左对齐。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="alignCenterIcon" /></div>
                <div class="icon-name">居中</div>
                <div class="icon-desc">设置当前列居中。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="alignRightIcon" /></div>
                <div class="icon-name">右对齐</div>
                <div class="icon-desc">设置当前列右对齐。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><Icon :icon="sparklesIcon" /></div>
                <div class="icon-name">格式化</div>
                <div class="icon-desc">格式化表格。</div>
              </div>
            </div>

            <div class="manual-subtitle">阅读器：工具栏图标与作用（Repository 右键“打开”后）</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ArrowBackOutline" /></div>
                <div class="icon-name">上一页</div>
                <div class="icon-desc">上一页（PageUp）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ArrowForwardOutline" /></div>
                <div class="icon-name">下一页</div>
                <div class="icon-desc">下一页（PageDown）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ExpandOutline" /></div>
                <div class="icon-name">适合宽度</div>
                <div class="icon-desc">将页面缩放到合适宽度（Ctrl+0）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="RemoveOutline" /></div>
                <div class="icon-name">缩小</div>
                <div class="icon-desc">缩小页面（Ctrl+-）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="AddOutline" /></div>
                <div class="icon-name">放大</div>
                <div class="icon-desc">放大页面（Ctrl+=）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="OptionsOutline" /></div>
                <div class="icon-name">切换布局</div>
                <div class="icon-desc">左右分栏 / 仅左侧 / 仅右侧 / 大纲（Ctrl+1/2/3/4）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="BookmarksOutline" /></div>
                <div class="icon-name">书签</div>
                <div class="icon-desc">添加/取消当前页书签。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="LibraryOutline" /></div>
                <div class="icon-name">书签列表</div>
                <div class="icon-desc">打开书签库并跳转到指定页。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="DocumentTextOutline" /></div>
                <div class="icon-name">笔记</div>
                <div class="icon-desc">显示/隐藏右侧“我的笔记”面板（Ctrl+N）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="LanguageOutline" /></div>
                <div class="icon-name">译文</div>
                <div class="icon-desc">显示/隐藏译文（Ctrl+T）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SearchOutline" /></div>
                <div class="icon-name">页内搜索</div>
                <div class="icon-desc">打开页内搜索（Ctrl+F）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="CloseOutline" /></div>
                <div class="icon-name">关闭</div>
                <div class="icon-desc">关闭页内搜索或关闭面板。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SaveOutline" /></div>
                <div class="icon-name">保存笔记</div>
                <div class="icon-desc">保存当前卡片编辑内容（Ctrl+S）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ExitOutline" /></div>
                <div class="icon-name">退出编辑</div>
                <div class="icon-desc">退出卡片编辑模式。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="TrashOutline" /></div>
                <div class="icon-name">删除</div>
                <div class="icon-desc">删除当前笔记卡片或清空书签。</div>
              </div>
            </div>

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
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelLeftContract24Regular" /></div>
                <div class="icon-name">左侧栏开关</div>
                <div class="icon-desc">切换左侧边栏（文件树/导航/分类）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelRightContract24Regular" /></div>
                <div class="icon-name">右侧栏开关</div>
                <div class="icon-desc">切换右侧边栏（详情/大纲/筛选/信息面板）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Search32Filled" /></div>
                <div class="icon-name">搜索</div>
                <div class="icon-desc">用于搜索文件或内容；资料库支持更细的搜索类型下拉选项。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Screenshot24Regular" /></div>
                <div class="icon-name">截图</div>
                <div class="icon-desc">首页为“截图并插入”；资料库为“截图保存到当前目录 images 文件夹”。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="BookAdd20Regular" /></div>
                <div class="icon-name">新建知识库</div>
                <div class="icon-desc">打开“创建知识库”弹窗。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="TextBulletListLtr20Filled" /></div>
                <div class="icon-name">大纲</div>
                <div class="icon-desc">打开/切换右侧大纲面板。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Box20Regular" /></div>
                <div class="icon-name">模板管理</div>
                <div class="icon-desc">打开模板管理器窗口。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="DocumentSearch20Filled" /></div>
                <div class="icon-name">搜索类型：文件</div>
                <div class="icon-desc">首页搜索下拉中选择“文件”。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="DocumentSearch24Regular" /></div>
                <div class="icon-name">搜索类型：笔记</div>
                <div class="icon-desc">首页搜索下拉中选择“笔记”。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="NotebookSection24Regular" /></div>
                <div class="icon-name">搜索类型：摘录</div>
                <div class="icon-desc">首页搜索下拉中选择“摘录”。</div>
              </div>
            </div>
          </div>

          <div class="page-section">
            <div class="page-title">首页（Home）</div>
            <div class="page-desc">
              首页包含顶部工具条与主内容区。顶部提供侧栏展开/收起、搜索、截图并插入、模板管理、分屏等能力；左侧通常为文件树/目录，右侧为辅助面板（如大纲/搜索导航），左右栏均支持拖拽调整宽度。
            </div>

            <div class="manual-subtitle">顶部栏图标与使用</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelLeftContract24Regular" /></div>
                <div class="icon-name">左侧栏开关</div>
                <div class="icon-desc">展开/收起左侧边栏。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Search32Filled" /></div>
                <div class="icon-name">搜索</div>
                <div class="icon-desc">搜索文件/内容/摘录（回车执行；可在下拉中切换搜索类型）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="BookAdd20Regular" /></div>
                <div class="icon-name">新建知识库</div>
                <div class="icon-desc">打开“创建知识库”弹窗。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="TextBulletListLtr20Filled" /></div>
                <div class="icon-name">大纲</div>
                <div class="icon-desc">打开/切换右侧大纲面板。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Box20Regular" /></div>
                <div class="icon-name">模板管理</div>
                <div class="icon-desc">打开模板管理器窗口，用于管理常用模板。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 24 24" width="18" height="18">
                      <path fill="currentColor" d="M4 5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v8.5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5zm2 0v8.5h12V5H6zm-1 14a1 1 0 0 1 1-1h4v-1h4v1h4a1 1 0 1 1 0 2H6a1 1 0 0 1-1-1z" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">PPT 制作与管理</div>
                <div class="icon-desc">打开 PPT 管理器，进行演示文稿创建与管理。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Folder20Regular" /></div>
                <div class="icon-name">知识库资源管理</div>
                <div class="icon-desc">打开资源库管理器，查看资源目录与同步状态。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Screenshot24Regular" /></div>
                <div class="icon-name">截图并插入</div>
                <div class="icon-desc">截图并将识别结果以 Markdown 插入到当前编辑器。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 16 16" width="16" height="16">
                      <rect x="1.5" y="2.5" width="13" height="11" rx="1" ry="1" fill="none" stroke="currentColor" stroke-width="1" />
                      <line x1="8" y1="3" x2="8" y2="13" stroke="currentColor" stroke-width="1" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">纵向分屏</div>
                <div class="icon-desc">将编辑区分为左右两个编辑器。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 16 16" width="16" height="16">
                      <rect x="1.5" y="2.5" width="13" height="11" rx="1" ry="1" fill="none" stroke="currentColor" stroke-width="1" />
                      <line x1="2" y1="8" x2="14" y2="8" stroke="currentColor" stroke-width="1" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">横向分屏</div>
                <div class="icon-desc">将编辑区分为上下两个编辑器。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelRightContract24Regular" /></div>
                <div class="icon-name">右侧栏开关</div>
                <div class="icon-desc">展开/收起右侧边栏。</div>
              </div>
            </div>

            <div class="manual-subtitle">左侧文件树工具条</div>
            <ul class="manual-ul">
              <li>浏览器预览：点击地球图标打开知识库浏览器预览入口。</li>
              <li>Git 操作：打开 Git 操作窗口，执行提交、拉取与推送等操作。</li>
              <li>刷新所有目录：重新加载知识库目录树。</li>
              <li>知识库管理器：打开知识库管理器窗口进行管理。</li>
            </ul>

            <div class="manual-subtitle">右侧面板：搜索导航</div>
            <ul class="manual-ul">
              <li>入口：在顶部搜索框回车执行搜索后，右侧会自动打开“搜索导航”面板。</li>
              <li>标签页：文件搜索结果 / 内容搜索结果 / 摘录，会随当前搜索类型自动切换。</li>
              <li>结果信息：展示文件名、行号与预览片段；命中内容会高亮显示。</li>
              <li>操作：单击结果可打开；支持 ↑/↓ 选择结果、Enter 打开当前结果。</li>
            </ul>

            <div class="manual-subtitle">常用窗口示意</div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('window-kb-manager')" alt="知识库管理器窗口" />
              </div>
              <div class="figure-caption">图 3-1：知识库管理器</div>
            </div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('window-template-manager')" alt="模板管理器窗口" />
              </div>
              <div class="figure-caption">图 3-2：模板管理器</div>
            </div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('window-ppt-manager')" alt="PPT 管理器窗口" />
              </div>
              <div class="figure-caption">图 3-3：PPT 管理器</div>
            </div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('window-resource-manager')" alt="知识库资源管理器窗口" />
              </div>
              <div class="figure-caption">图 3-4：知识库资源管理器</div>
            </div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('window-git-panel')" alt="Git 操作窗口" />
              </div>
              <div class="figure-caption">图 3-5：Git 操作窗口</div>
            </div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('window-browser-preview')" alt="浏览器预览窗口" />
              </div>
              <div class="figure-caption">图 3-6：浏览器预览</div>
            </div>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-home')" alt="首页布局" />
              </div>
              <div class="figure-caption">图 3：首页</div>
            </div>
          </div>

          <div id="repository" class="page-section">
            <div class="page-title">资料库（Repository）</div>
            <div class="page-desc">
              该页面用于管理导入的本地资料文件。你可以导入文件并补充元数据（文件库/知识库/标签/描述），通过顶部搜索快速定位；在表格中右键条目可打开阅读器窗口进行阅读与做笔记。
              建议在导入时完善标签与描述，便于后续搜索与统计分析。
            </div>

            <div class="manual-subtitle">导入文件（重点）</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="FolderAdd20Regular" /></div>
                <div class="icon-name">导入文件</div>
                <div class="icon-desc">打开“导入文件”弹窗并提交入库。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SortAscendingIcon" /></div>
                <div class="icon-name">排序</div>
                <div class="icon-desc">按文件名 / 所属文件库 / 入库时间排序。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="FilterOffIcon" /></div>
                <div class="icon-name">清除筛选</div>
                <div class="icon-desc">清除表格所有筛选条件。</div>
              </div>
            </div>

            <div class="manual-subtitle">顶部栏图标与搜索</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelLeftContract24Regular" /></div>
                <div class="icon-name">左侧栏开关</div>
                <div class="icon-desc">展开/收起左侧边栏。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Search32Filled" /></div>
                <div class="icon-name">搜索</div>
                <div class="icon-desc">搜索导入文件（回车执行）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="DocumentSearch24Regular" /></div>
                <div class="icon-name">搜索类型</div>
                <div class="icon-desc">在下拉中切换：按文件名/描述/标签/文件库/知识库/类型搜索。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="TextBulletListLtr20Filled" /></div>
                <div class="icon-name">列表入口</div>
                <div class="icon-desc">当前实现为静态图标（无点击动作）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Screenshot24Regular" /></div>
                <div class="icon-name">截图</div>
                <div class="icon-desc">将截图保存到当前文件同目录下的 images 文件夹（.png）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelRightContract24Regular" /></div>
                <div class="icon-name">右侧栏开关</div>
                <div class="icon-desc">展开/收起右侧边栏。</div>
              </div>
            </div>

            <div class="manual-subtitle">后台任务与批量操作（工具条）</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="TasksApp20Regular" /></div>
                <div class="icon-name">任务列表</div>
                <div class="icon-desc">打开转换/备份/导入/导出等任务列表。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SaveSync20Regular" /></div>
                <div class="icon-name">备份资料库</div>
                <div class="icon-desc">下拉选择“备份当前库 / 备份所有库”。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="BoxArrowLeft20Regular" /></div>
                <div class="icon-name">从备份恢复</div>
                <div class="icon-desc">执行资料库恢复任务。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="TextBulletListAdd20Filled" /></div>
                <div class="icon-name">导入条目</div>
                <div class="icon-desc">执行条目批量导入任务。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SignOut20Regular" /></div>
                <div class="icon-name">导出条目</div>
                <div class="icon-desc">执行条目批量导出任务。</div>
              </div>
            </div>

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
                <img class="figure-img" :src="manualImage('window-reader')" alt="阅读器窗口" />
              </div>
              <div class="figure-caption">图 4-1：阅读器窗口</div>
            </div>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-repo-search')" alt="资料库搜索" />
              </div>
              <div class="figure-caption">图 4：资料库搜索</div>
            </div>
          </div>

          <div id="flipboard" class="page-section">
            <div class="page-title">资讯获取</div>
            <div class="page-desc">
              资讯获取页面采用三栏布局：左侧分类/列表，中间列表视图，右侧详情阅读面板。左右栏可折叠与拖拽调整宽度；
              选择条目后会在右侧加载咨询详情。
            </div>

            <div class="manual-subtitle">顶部栏图标与使用</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelLeftContract24Regular" /></div>
                <div class="icon-name">左侧栏开关</div>
                <div class="icon-desc">切换左侧分类/列表侧栏。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SearchOutline" /></div>
                <div class="icon-name">搜索</div>
                <div class="icon-desc">搜索文章、主题（输入框前缀图标）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="NewspaperOutline" /></div>
                <div class="icon-name">杂志视图</div>
                <div class="icon-desc">切换为 magazine 模式。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="GridOutline" /></div>
                <div class="icon-name">网格视图</div>
                <div class="icon-desc">切换为 grid 模式。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ListOutline" /></div>
                <div class="icon-name">列表视图</div>
                <div class="icon-desc">切换为 list 模式。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="RefreshOutline" /></div>
                <div class="icon-name">刷新</div>
                <div class="icon-desc">刷新文章列表。</div>
              </div>
            </div>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-flipboard')" alt="资讯获取页面三栏布局" />
              </div>
              <div class="figure-caption">图 5：资讯获取页面</div>
            </div>
          </div>

          <div id="chat" class="page-section">
            <div class="page-title">聊天（Chat）</div>
            <div class="page-desc">
              聊天页面用于与大模型对话。支持会话管理、会话内消息搜索、翻译模式、附件上传、多选消息发送到编辑器等能力。
              输入区已与消息区视觉融合，滚动不在底部时会显示“回到底部”按钮；切换页面后返回聊天页，会继续渲染当前会话消息流。
            </div>
            <div class="manual-subtitle">聊天页顶部与会话列表</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SearchOutline" /></div>
                <div class="icon-name">搜索会话</div>
                <div class="icon-desc">左侧会话列表顶部输入框，可按会话标题、首条用户消息或会话消息内容过滤。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
                      <path d="M19 11h-6V5h-2v6H5v2h6v6h2v-6h6z" fill="currentColor" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">新建会话</div>
                <div class="icon-desc">在会话列表创建新会话（鼠标悬停提示“新建会话”）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SearchOutline" /></div>
                <div class="icon-name">搜索当前会话消息</div>
                <div class="icon-desc">聊天页顶部搜索框，按关键字过滤当前会话消息（匹配原始 Markdown 与渲染文本）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="OpenOutline" /></div>
                <div class="icon-name">打开网页模型窗口</div>
                <div class="icon-desc">当模型为网页模式（如元宝/ChatGPT 网页）时，可快速打开对应网页窗口。</div>
              </div>
            </div>

            <div class="manual-subtitle">消息区操作图标</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
                      <path d="M9 11l3 3L22 4l-1.4-1.4L12 11.2 10.4 9.6zM2 6h8V4H2zm0 6h5v-2H2zm0 6h8v-2H2z" fill="currentColor" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">多选模式</div>
                <div class="icon-desc">位于聊天标题栏右侧；进入后可勾选多条消息，并执行“全选可见 / 清空 / 发送到编辑器”。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="CopyOutline" /></div>
                <div class="icon-name">复制消息</div>
                <div class="icon-desc">用户消息和模型消息均支持复制；模型消息复制优先返回原始 Markdown 内容。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ShareSocialOutline" /></div>
                <div class="icon-name">分享（模型消息）</div>
                <div class="icon-desc">分享当前模型回复内容。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="RefreshOutline" /></div>
                <div class="icon-name">重新生成（模型消息）</div>
                <div class="icon-desc">以当前上下文重新请求大模型生成回复。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="ChevronDownOutline" /></div>
                <div class="icon-name">回到底部</div>
                <div class="icon-desc">当未停留在最新消息底部时显示，位于输入框上方中间；点击快速跳转到最后一条消息。</div>
              </div>
            </div>

            <div class="manual-subtitle">输入区图标与模式</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
                      <path d="M17.5 6.5v9a5.5 5.5 0 0 1-11 0v-10a4 4 0 0 1 8 0v9a2.5 2.5 0 1 1-5 0V8h1.8v6.5a.7.7 0 1 0 1.4 0v-9a2.2 2.2 0 0 0-4.4 0v10a3.7 3.7 0 0 0 7.4 0v-9h1.8z" fill="currentColor" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">添加附件</div>
                <div class="icon-desc">为当前提问附加文件路径；已选附件会显示在输入框上方，可单独移除。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="LanguageOutline" /></div>
                <div class="icon-name">翻译模式</div>
                <div class="icon-desc">点击进入/退出翻译模式（图标高亮表示已开启）。开启后发送按钮仍用于发送消息，但模型将按目标语言返回译文。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline">
                  <n-icon>
                    <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
                      <path d="M12 4l-7 7 1.4 1.4 4.6-4.6V20h2V7.8l4.6 4.6L19 11z" fill="currentColor" />
                    </svg>
                  </n-icon>
                </div>
                <div class="icon-name">发送</div>
                <div class="icon-desc">发送输入内容；输入框默认至少显示三行高度，支持 Shift+Enter 换行。</div>
              </div>
            </div>

            <div class="manual-subtitle">会话行为说明</div>
            <ul class="manual-ul">
              <li>会话标题会在回复后自动生成摘要标题；你也可以在聊天标题处双击手动重命名并锁定标题。</li>
              <li>左侧会话列表的小字预览优先显示该会话首条用户消息，而非模型欢迎语。</li>
              <li>“发送到编辑器”会把多选消息写入首页当前激活标签文件的光标位置。</li>
            </ul>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-chat')" alt="聊天页面与模型选择" />
              </div>
              <div class="figure-caption">图 6：聊天</div>
            </div>
          </div>

          <div id="brain" class="page-section">
            <div class="page-title">知识库（Brain）</div>
            <div class="page-desc">
              知识库页面用于创建与管理知识库内容，并提供“创建知识库”表单入口。建议先在设置中配置知识库根目录，
              再在此页面创建新的知识库结构。
            </div>

            <div class="manual-subtitle">图谱工具条图标（Knowledge Graph）</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="CompassOutline" /></div>
                <div class="icon-name">重置视角</div>
                <div class="icon-desc">将图谱视角恢复到默认状态。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SyncOutline" /></div>
                <div class="icon-name">自动旋转</div>
                <div class="icon-desc">开启/停止图谱自动旋转。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="MoonOutline" /></div>
                <div class="icon-name">暗色模式</div>
                <div class="icon-desc">切换亮色/暗色模式。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="LayersOutline" /></div>
                <div class="icon-name">布局设置</div>
                <div class="icon-desc">打开布局模式选择（popover）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SpeedometerOutline" /></div>
                <div class="icon-name">性能设置</div>
                <div class="icon-desc">调整渲染质量/性能档位（slider）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="SparklesOutline" /></div>
                <div class="icon-name">视觉特效</div>
                <div class="icon-desc">星光/光晕等视觉特效开关与参数。</div>
              </div>
            </div>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-brain')" alt="知识库管理与创建" />
              </div>
              <div class="figure-caption">图 7：知识库</div>
            </div>
          </div>

          <div id="statistics" class="page-section">
            <div class="page-title">统计（Statistics）</div>
            <div class="page-desc">
              统计看板用于查看知识库与资料库的趋势与活跃度，支持选择时间范围与手动刷新（Beta）。
              建议结合时间范围筛选观察近期产出与阅读变化。
            </div>
            <div class="manual-subtitle">主要面板</div>
            <ul class="manual-ul">
              <li>知识量趋势：按知识库统计文件/目录数量，支持筛选知识库与指标。</li>
              <li>活跃热力图：展示知识库 Git 提交的年度活跃热力分布。</li>
              <li>阅读进度：展示阅读器记录的完成度与耗时汇总。</li>
              <li>资料库文档：按导入文件标签统计分布，可切换知识库。</li>
            </ul>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-statistics')" alt="统计看板" />
              </div>
              <div class="figure-caption">图 8：统计看板</div>
            </div>
          </div>

          <div id="whiteboard" class="page-section">
            <div class="page-title">白板（Whiteboard）</div>
            <div class="page-desc">
              白板用于快速绘制草图、流程图与结构图，基于 Excalidraw 交互体验。
              适用于头脑风暴、结构梳理与临时标注。
            </div>
            <div class="manual-subtitle">使用要点</div>
            <ul class="manual-ul">
              <li>绘图：选择形状、文本、连线等工具进行绘制与标注。</li>
              <li>自动保存：编辑内容会自动保存到本地，重开页面可恢复最近内容。</li>
            </ul>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-whiteboard')" alt="白板页面" />
              </div>
              <div class="figure-caption">图 9：白板</div>
            </div>
          </div>

          <div id="application" class="page-section">
            <div class="page-title">应用（Application）</div>
            <div class="page-desc">
              应用页面提供可打开的工具/应用集合：左侧为分类（工具/语言/音乐/工作/生活），中心为应用卡片列表；
              点击卡片会打开独立窗口。示例应用包含相册、TO DO、翻译助手、文件传输助手、Python Playground 等，
              部分应用仍为占位实现。右侧区域可用于展示应用详情或扩展信息（以当前界面为准）。
            </div>
            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-application')" alt="应用中心" />
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

            <div class="manual-subtitle">顶部栏图标与使用</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelLeftContract24Regular" /></div>
                <div class="icon-name">左侧栏开关</div>
                <div class="icon-desc">切换左侧设置菜单栏。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="PanelRightContract24Regular" /></div>
                <div class="icon-name">右侧栏开关</div>
                <div class="icon-desc">切换右侧扩展面板。</div>
              </div>
            </div>

            <div class="manual-subtitle">左侧菜单图标（Setting Sidebar）</div>
            <div class="icon-block">
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="FolderAdd20Regular" /></div>
                <div class="icon-name">笔记库</div>
                <div class="icon-desc">配置文件库根目录/资源库根目录/知识库根目录等。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Image28Regular" /></div>
                <div class="icon-name">外观</div>
                <div class="icon-desc">外观与偏好设置（以当前实现为准）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Server24Regular" /></div>
                <div class="icon-name">服务器</div>
                <div class="icon-desc">服务器相关配置（以当前实现为准）。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="Key24Regular" /></div>
                <div class="icon-name">大模型</div>
                <div class="icon-desc">配置各家模型 API Key 及 Markdown 转换模型/运行环境。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="InformationCircleOutline" /></div>
                <div class="icon-name">关于</div>
                <div class="icon-desc">关于信息与版本说明。</div>
              </div>
              <div class="icon-row">
                <div class="icon-inline"><n-icon :component="BookOutline" /></div>
                <div class="icon-name">用户手册</div>
                <div class="icon-desc">打开本手册页面。</div>
              </div>
            </div>

            <div class="figure">
              <div class="figure-box">
                <img class="figure-img" :src="manualImage('page-setting')" alt="设置页面与菜单" />
              </div>
              <div class="figure-caption">图 9：设置页面</div>
            </div>
          </div>
        </section>

        <n-divider />

        <section id="setting">
          <h2 class="h2">设置：大模型与 Markdown 转换</h2>
          <p class="p">
            本节说明大模型与转换能力的安装与配置流程，配置完成后即可在聊天与转换流程中使用。
          </p>

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
              <img class="figure-img" :src="manualImage('setting-llm')" alt="大模型与转换配置" />
            </div>
            <div class="figure-caption">图 10：大模型与转换配置</div>
          </div>

          <h3 class="h3">本地模型（LocalDeepseek）</h3>
          <p class="p">
            本地模型用于离线或内网环境下的推理场景，在同一页面可完成下载、导入与状态检查。
          </p>
          <ul class="manual-ul">
            <li>下载：点击“下载”获取模型文件，完成后状态显示已安装。</li>
            <li>导入：已有模型包时点击“导入”并选择本地文件。</li>
          </ul>
          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('setting-local-llm')" alt="本地模型配置" />
            </div>
            <div class="figure-caption">图 11：本地模型配置</div>
          </div>
        </section>

        <n-divider />

        <section id="faq">
          <h2 class="h2">常见问题</h2>

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
import { computed, h } from 'vue'
import { Icon } from '@iconify/vue'
import { NCard, NSpace, NAlert, NDivider, NTag, NIcon } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import {
  BookAdd20Regular,
  Box20Regular,
  BoxArrowLeft20Regular,
  DocumentEdit16Regular,
  DocumentSearch20Filled,
  DocumentSearch24Regular,
  Folder20Regular,
  FolderAdd20Regular,
  NotebookSection24Regular,
  PanelLeftContract24Regular,
  PanelRightContract24Regular,
  SaveSync20Regular,
  Screenshot24Regular,
  Search32Filled,
  SignOut20Regular,
  TasksApp20Regular,
  TextBulletListAdd20Filled,
  TextBulletListLtr20Filled,
  Edit16Filled
} from '@vicons/fluent'
import {
  AddOutline,
  ArrowBackOutline,
  ArrowForwardOutline,
  BookmarksOutline,
  ChevronDownOutline,
  CloseOutline,
  CompassOutline,
  CopyOutline,
  DocumentTextOutline,
  ExitOutline,
  ExpandOutline,
  GridOutline,
  LanguageOutline,
  LayersOutline,
  LibraryOutline,
  ListOutline,
  LogoVimeo,
  MoonOutline,
  NewspaperOutline,
  OpenOutline,
  OptionsOutline,
  RefreshOutline,
  RemoveOutline,
  SaveOutline,
  SearchOutline,
  ShareSocialOutline,
  SparklesOutline,
  SpeedometerOutline,
  SyncOutline,
  TrashOutline
} from '@vicons/ionicons5'

// Iconify local icons to preserve original lucide/mdi visuals offline
import filePenLineIcon from '@iconify-icons/lucide/file-edit'
import eyeIcon from '@iconify-icons/lucide/eye'
import columns2Icon from '@iconify-icons/lucide/columns'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import minusIcon from '@iconify-icons/lucide/minus'
import undoIcon from '@iconify-icons/lucide/undo'
import redoIcon from '@iconify-icons/lucide/redo'
import boldIcon from '@iconify-icons/lucide/bold'
import italicIcon from '@iconify-icons/lucide/italic'
import strikethroughIcon from '@iconify-icons/lucide/strikethrough'
import linkIcon from '@iconify-icons/lucide/link'
import codeIcon from '@iconify-icons/lucide/code'
import quoteIcon from '@iconify-icons/lucide/quote'
import tableRowPlusAfter from '@iconify-icons/mdi/table-row-plus-after'
import tableRowRemove from '@iconify-icons/mdi/table-row-remove'
import tableColumnPlusAfter from '@iconify-icons/mdi/table-column-plus-after'
import tableColumnRemove from '@iconify-icons/mdi/table-column-remove'
import alignLeftIcon from '@iconify-icons/lucide/align-left'
import alignCenterIcon from '@iconify-icons/lucide/align-center'
import alignRightIcon from '@iconify-icons/lucide/align-right'

const settingsStore = useSettingsStore()

const isDarkTheme = computed(() => {
  const storeTheme = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  if (storeTheme === 'dark') return true
  if (storeTheme === 'light') return false
  const dataTheme = document?.documentElement?.dataset?.guaxiTheme
  return dataTheme === 'dark'
})

const manualImages = {
  overview: {
    light: new URL('../../assets/manual/overview.png', import.meta.url).href,
    dark: new URL('../../assets/manual/overview-dark.png', import.meta.url).href
  },
  'setting-noterepo': {
    light: new URL('../../assets/manual/setting-noterepo.png', import.meta.url).href,
    dark: new URL('../../assets/manual/setting-noterepo-dark.png', import.meta.url).href
  },
  'window-kb-manager': {
    light: new URL('../../assets/manual/window-kb-manager.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-kb-manager-dark.png', import.meta.url).href
  },
  'window-template-manager': {
    light: new URL('../../assets/manual/window-template-manager.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-template-manager-dark.png', import.meta.url).href
  },
  'window-ppt-manager': {
    light: new URL('../../assets/manual/window-ppt-manager.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-ppt-manager-dark.png', import.meta.url).href
  },
  'window-resource-manager': {
    light: new URL('../../assets/manual/window-resource-manager.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-resource-manager-dark.png', import.meta.url).href
  },
  'window-git-panel': {
    light: new URL('../../assets/manual/window-git-panel.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-git-panel-dark.png', import.meta.url).href
  },
  'window-browser-preview': {
    light: new URL('../../assets/manual/window-browser-preview.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-browser-preview-dark.png', import.meta.url).href
  },
  'page-home': {
    light: new URL('../../assets/manual/page-home.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-home-dark.png', import.meta.url).href
  },
  'window-reader': {
    light: new URL('../../assets/manual/window-reader.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-reader-dark.png', import.meta.url).href
  },
  'page-repo-search': {
    light: new URL('../../assets/manual/page-repo-search.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-repo-search-dark.png', import.meta.url).href
  },
  'page-flipboard': {
    light: new URL('../../assets/manual/page-flipboard.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-flipboard-dark.png', import.meta.url).href
  },
  'page-chat': {
    light: new URL('../../assets/manual/page-chat.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-chat-dark.png', import.meta.url).href
  },
  'page-brain': {
    light: new URL('../../assets/manual/page-brain.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-brain-dark.png', import.meta.url).href
  },
  'page-statistics': {
    light: new URL('../../assets/manual/page-statistics.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-statistics-dark.png', import.meta.url).href
  },
  'page-whiteboard': {
    light: new URL('../../assets/manual/page-whiteboard.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-whiteboard-dark.png', import.meta.url).href
  },
  'page-application': {
    light: new URL('../../assets/manual/page-application.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-application-dark.png', import.meta.url).href
  },
  'page-setting': {
    light: new URL('../../assets/manual/page-setting.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-setting-dark.png', import.meta.url).href
  },
  'setting-llm': {
    light: new URL('../../assets/manual/setting-llm.png', import.meta.url).href,
    dark: new URL('../../assets/manual/setting-llm-dark.png', import.meta.url).href
  },
  'setting-local-llm': {
    light: new URL('../../assets/manual/setting-local-llm-light.png', import.meta.url).href,
    dark: new URL('../../assets/manual/setting-local-llm-dark.png', import.meta.url).href
  }
}

const manualImage = (key) => {
  const entry = manualImages[String(key || '')]
  if (!entry) return ''
  return isDarkTheme.value ? entry.dark : entry.light
}

const SortAscendingIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M19 17h3l-4 4-4-4h3V3h2v14zM9 13H5v-2h4V9H5V7h4V5H5V3H3v12h2v-2h4v-2zm2-2h2v2h-2v-2zm0-4h2v2h-2V7zm0 8h2v2h-2v-2z'
      })
    ])
  }
}

const FilterOffIcon = {
  render() {
    return h('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 24 24',
      fill: 'currentColor'
    }, [
      h('path', {
        d: 'M19.79 5.61C20.3 4.95 19.83 4 19 4H6.83l7.97 7.97 4.99-6.36zM2.81 2.81L1.39 4.22 10 13v6c0 .55.45 1 1 1h2c.55 0 1-.45 1-1v-2.17l5.78 5.78 1.41-1.41L2.81 2.81z'
      })
    ])
  }
}

const scrollToSection = (id) => {
  const el = document.getElementById(String(id))
  if (!el) return
  el.scrollIntoView({ behavior: 'smooth', block: 'start' })
}
</script>

<style scoped>
.manual {
  width: 100%;
  box-sizing: border-box;
}

.icon-inline {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  vertical-align: -3px;
  margin-right: 6px;
  flex: 0 0 auto;
}

.icon-inline :deep(svg) {
  display: block;
}

.icon-inline :deep(.n-icon) {
  --n-size: 18px;
}

.icon-block {
  margin: 8px 0 0;
  display: grid;
  gap: 6px;
}

.icon-row {
  display: grid;
  grid-template-columns: 22px 140px 1fr;
  gap: 10px;
  align-items: start;
  padding: 6px 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
}

.icon-name {
  font-weight: 600;
  line-height: 1.6;
}

.icon-desc {
  color: var(--guaxi-text-secondary);
  line-height: 1.6;
}

.icon-inline {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  vertical-align: -3px;
  margin-right: 6px;
  flex: 0 0 auto;
}

.icon-inline :deep(svg) {
  display: block;
}

.icon-inline :deep(.n-icon) {
  --n-size: 18px;
}

.icon-block {
  margin: 8px 0 0;
  display: grid;
  gap: 6px;
}

.icon-row {
  display: grid;
  grid-template-columns: 22px 140px 1fr;
  gap: 10px;
  align-items: start;
  padding: 6px 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
}

.icon-name {
  font-weight: 600;
  line-height: 1.6;
}

.icon-desc {
  color: var(--guaxi-text-secondary);
  line-height: 1.6;
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

.manual section {
  scroll-margin-top: 12px;
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

.toc-list {
  margin: 0;
  padding-left: 18px;
}

.toc-link {
  appearance: none;
  border: 0;
  padding: 0;
  background: transparent;
  color: var(--n-color-primary);
  cursor: pointer;
  text-align: left;
  font: inherit;
}

.toc-link:hover {
  text-decoration: underline;
}

.toc-link:focus-visible {
  outline: 2px solid rgba(64, 158, 255, 0.5);
  outline-offset: 2px;
  border-radius: 4px;
}

.figure {
  margin-top: 12px;
}

.figure-box {
  border: 1px dashed var(--guaxi-border);
  border-radius: 12px;
  min-height: 440px;
  padding: 0;
  display: flex;
  flex-direction: column;
  justify-content: stretch;
  align-items: stretch;
  background: rgba(127, 127, 127, 0.08);
  user-select: none;
  overflow: hidden;
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
  height: 100%;
  max-height: 80vh;
  object-fit: cover;
  border-radius: 0;
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
  appearance: none;
  cursor: pointer;
  text-align: left;
  width: 100%;
  font: inherit;
}

.feature-card:hover {
  border-color: var(--n-color-primary);
}

.feature-card:focus-visible {
  outline: 2px solid rgba(64, 158, 255, 0.5);
  outline-offset: 2px;
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

:root[data-guaxi-theme='dark'] .manual .h2,
:root[data-guaxi-theme='dark'] .manual .h3,
:root[data-guaxi-theme='dark'] .manual .page-title,
:root[data-guaxi-theme='dark'] .manual .manual-subtitle,
:root[data-guaxi-theme='dark'] .manual .feature-title,
:root[data-guaxi-theme='dark'] .manual .toc-title,
:root[data-guaxi-theme='dark'] .manual .icon-name,
:root[data-guaxi-theme='dark'] .manual .q {
  color: #f0f6fc;
}

:root[data-guaxi-theme='dark'] .manual .p,
:root[data-guaxi-theme='dark'] .manual .page-desc,
:root[data-guaxi-theme='dark'] .manual .feature-desc,
:root[data-guaxi-theme='dark'] .manual .icon-desc,
:root[data-guaxi-theme='dark'] .manual .manual-ul,
:root[data-guaxi-theme='dark'] .manual .ol,
:root[data-guaxi-theme='dark'] .manual .a,
:root[data-guaxi-theme='dark'] .manual .figure-caption,
:root[data-guaxi-theme='dark'] .manual .figure-sub {
  color: #c9d1d9;
}

.dark-mode .manual .h2,
.dark-mode .manual .h3,
.dark-mode .manual .page-title,
.dark-mode .manual .manual-subtitle,
.dark-mode .manual .feature-title,
.dark-mode .manual .toc-title,
.dark-mode .manual .icon-name,
.dark-mode .manual .q {
  color: #f0f6fc;
}

.dark-mode .manual .p,
.dark-mode .manual .page-desc,
.dark-mode .manual .feature-desc,
.dark-mode .manual .icon-desc,
.dark-mode .manual .manual-ul,
.dark-mode .manual .ol,
.dark-mode .manual .a,
.dark-mode .manual .figure-caption,
.dark-mode .manual .figure-sub {
  color: #c9d1d9;
}
</style>
