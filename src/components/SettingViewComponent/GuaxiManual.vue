<template>
  <div class="manual">
    <n-card title="用户手册" size="small">
      <n-space vertical size="large" style="width: 100%">
        <n-alert type="info" :bordered="false">
          本手册按当前代码中的页面入口和独立窗口重新整理，覆盖首页、资料库、聊天、知识图谱、知识源、任务、应用、统计、白板与设置。
        </n-alert>

        <div class="toc">
          <div class="toc-title">目录</div>
          <ul class="toc-list">
            <li><button type="button" class="toc-link" @click="scrollToSection('intro')">软件简介</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('features')">当前功能版图</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('quickstart')">快速开始</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('home')">首页与编辑器</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('reader')">资料库与阅读器</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('chat')">聊天与模型</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('brain')">知识图谱</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('cognition')">大脑（认知层）</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('search')">知识源与收集</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('task')">任务管理</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('apps')">应用中心</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('setting')">设置</button></li>
            <li><button type="button" class="toc-link" @click="scrollToSection('faq')">常见问题</button></li>
          </ul>
        </div>

        <n-divider />

        <section id="intro">
          <h2 class="h2">软件简介</h2>
          <p class="p">
            Guaxi 是一款面向个人知识工作流的桌面软件。它把本地文件、Markdown 编辑、资料阅读、AI 对话、知识图谱、网络知识源、任务记录和工具应用放在同一套界面里，
            适合完成“导入资料 → 阅读摘录 → AI 辅助理解 → 写作整理 → 知识库沉淀 → 统计复盘”的长期循环。
          </p>
          <p class="p">
            当前版本的核心不再只是“资料库 + 聊天 + 知识库”，而是扩展为一个工作台：左侧导航进入主要页面，顶部工具栏打开独立窗口，设置页统一管理目录、模型、同步、终端和语音能力。
          </p>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('overview')" alt="软件主界面" />
            </div>
            <div class="figure-caption">图 1：Guaxi 主界面总览</div>
          </div>
        </section>

        <n-divider />

        <section id="features">
          <h2 class="h2">当前功能版图</h2>
          <div class="feature-grid">
            <button type="button" class="feature-card" @click="scrollToSection('home')">
              <div class="feature-title">知识编辑工作台</div>
              <div class="feature-desc">知识库文件树、任务目录、普通目录、打开历史、Monaco 编辑器、预览/分屏、右侧大纲/助手/搜索面板。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('reader')">
              <div class="feature-title">资料库与阅读器</div>
              <div class="feature-desc">管理导入资料，打开 PDF、Word、Markdown、文本、EPUB、HTML 等阅读器，支持页内搜索、摘录、翻译、笔记和助手。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('chat')">
              <div class="feature-title">聊天与模型</div>
              <div class="feature-desc">多会话、会话搜索、重命名/保存/导出、消息多选发送到编辑器、附件、翻译模式、本地模型与云端模型切换。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('brain')">
              <div class="feature-title">知识图谱</div>
              <div class="feature-desc">知识库树、3D 图谱、节点类型/标签筛选、语义搜索、最短路径、社区折叠、AI 概念/关系/标签建议。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('cognition')">
              <div class="feature-title">大脑（认知层）</div>
              <div class="feature-desc">后台读你已经拥有的材料，攒出主张、发现矛盾与缺口，只在够重要时才提醒你；不够重要的降级进每日简报。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('search')">
              <div class="feature-title">知识源聚合</div>
              <div class="feature-desc">RSS/网页/平台来源配置，主题抓取，网格/列表/杂志视图，收藏、原文阅读、登录队列与收集面板。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('task')">
              <div class="feature-title">任务管理</div>
              <div class="feature-desc">工作/生活/学习分类，看板、日历、统计，Org/JSON 导入，报表导出，任务库 Git 同步。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('apps')">
              <div class="feature-title">应用与工具</div>
              <div class="feature-desc">相册、TO DO、音乐、翻译、文件传输、Python Playground、有声书/配音工坊，以及数据与数学工作区。</div>
            </button>
            <button type="button" class="feature-card" @click="scrollToSection('setting')">
              <div class="feature-title">设置与同步</div>
              <div class="feature-desc">目录、主题、语音、LSP、终端、文件传输、GC、大模型、Markdown 转换、本地模型、P2P/WebDAV/Syncthing。</div>
            </button>
          </div>
        </section>

        <n-divider />

        <section id="quickstart">
          <h2 class="h2">快速开始</h2>
          <h3 class="h3">1）先配置目录</h3>
          <ol class="ol">
            <li>进入 <n-tag size="small">设置</n-tag> → <n-tag size="small">笔记库</n-tag>，配置知识库根目录、资源库根目录、模板库路径和 Slidev PPT 源文件目录。</li>
            <li>如果使用资料导入和阅读器，确认文件库/资源库路径可读写；截图、图片和摘录会依赖资源库或图床目录。</li>
            <li>进入 <n-tag size="small">设置</n-tag> → <n-tag size="small">任务管理</n-tag>，配置任务库根目录并点击“初始化目录”，系统会准备工作、生活、学习三类任务目录。</li>
          </ol>

          <h3 class="h3">2）再配置模型与转换能力</h3>
          <ol class="ol">
            <li>进入 <n-tag size="small">设置</n-tag> → <n-tag size="small">大模型</n-tag>，按需填写 ChatGPT、DeepSeek、Qwen、Kimi、豆包、硅基流动或自定义模型的 Base URL、API Key、模型名。</li>
            <li>设置 ChatRecord 保存路径，未设置时默认使用软件同级目录下的 ChatRecord。</li>
            <li>如需 PDF/文档转换，安装 Markdown 转换模型和转换运行环境；如需离线推理，下载或导入 LocalDeepseek / LocalQwen。</li>
          </ol>

          <h3 class="h3">3）按目标进入页面</h3>
          <ol class="ol">
            <li>写作整理：进入首页，选择知识库或目录，在 Monaco 编辑器中编辑 Markdown。</li>
            <li>阅读资料：进入资料库，打开文件进入阅读器，使用摘录、翻译、笔记和 AI 助手。</li>
            <li>知识源阅读：进入知识源页面，配置来源或搜索主题，收藏并插入到编辑器。</li>
            <li>长期复盘：进入统计页查看知识量趋势、活跃热力图、阅读进度和资料库标签分布。</li>
          </ol>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('setting-noterepo')" alt="设置-目录配置" />
            </div>
            <div class="figure-caption">图 2：目录与库配置</div>
          </div>
        </section>

        <n-divider />

        <section id="home">
          <h2 class="h2">首页与编辑器</h2>
          <p class="p">
            首页是主要工作台。左侧栏可在知识库树、任务目录、普通目录和打开历史之间工作；中间是编辑器；右侧栏承载大纲、智能助手、搜索结果和数学工作区等辅助面板。
          </p>

          <h3 class="h3">顶部工具栏</h3>
          <ul class="manual-ul">
            <li>全局搜索：搜索文件、正文关键字和摘录笔记，支持时间段、文件类型、自定义后缀和标签筛选；可用 Ctrl+P 聚焦搜索框。</li>
            <li>知识库与工具窗口：可新建知识库、打开大纲、智能助手、模板管理、PPT 制作与管理、知识库资源管理、任务管理、语言学习窗口。</li>
            <li>编辑增强：支持截图并插入、纵向/横向分屏、切换左右侧栏。</li>
            <li>资源库同步：启动/关闭时会尝试执行资源库自动同步，并记录同步状态。</li>
          </ul>

          <h3 class="h3">文件树与目录</h3>
          <ul class="manual-ul">
            <li>知识库树：支持新建文件/文件夹、导入 Markdown、重命名、删除、移动、在文件浏览器中打开、用 Neovim 打开、在浏览器中查看。</li>
            <li>任务目录：显示工作、生活、学习目录，可快速导入、创建文件/文件夹、用 Neovim 打开。</li>
            <li>普通目录：可打开本地目录作为临时工作区，保留打开历史。</li>
            <li>Git 面板：可从文件树打开当前工作区 Git 窗口，执行状态查看、暂存/取消暂存、提交、推送、拉取、分支切换、差异查看和 hunk 级操作。</li>
          </ul>

          <h3 class="h3">编辑器能力</h3>
          <ul class="manual-ul">
            <li>支持 Markdown 编辑、预览、分屏预览和实时预览；常用快捷键包括 Ctrl+S 保存、Ctrl+F 搜索、Ctrl+1/2/3/4 切换模式。</li>
            <li>选中文本后可使用浮动工具条加粗、斜体、删除线、链接、行内代码和引用。</li>
            <li>光标在 Markdown 表格内时会显示表格工具条，支持增删行列、设置对齐和格式化。</li>
            <li>右键菜单包含剪切/复制/粘贴、文件内搜索、Markdown 全文/选区格式化、编辑位置跳转、插入 Markdown 模板和命令面板。</li>
            <li>Python 代码块、数据表格和数学内容可配合数据实验室、数学工作区和终端面板完成运行、分析、绘图与报告写回。</li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-home')" alt="首页与编辑器" />
            </div>
            <div class="figure-caption">图 3：首页编辑工作台</div>
          </div>
        </section>

        <n-divider />

        <section id="reader">
          <h2 class="h2">资料库与阅读器</h2>
          <p class="p">
            资料库页面用于管理导入资料，阅读器用于深度阅读。当前阅读链路覆盖 PDF、转换后的 PDF/Markdown、EPUB、HTML、Markdown、文本、Word 等资料类型。
          </p>
          <ul class="manual-ul">
            <li>资料库可搜索、定位和打开导入文件，打开后根据文件类型进入对应阅读器。</li>
            <li>阅读器支持上一页/下一页、缩放、适合宽度、布局切换、书签、页内搜索、目录/缩略图和阅读状态记录。</li>
            <li>选中文本后可翻译、查询、问 AI、用 AI 翻译或插入笔记；相关结果会写入学习记录或摘录笔记。</li>
            <li>PDF 阅读器支持高亮、标注、删除高亮、保存回原 PDF；右侧“我的笔记”可新增、编辑、删除卡片。</li>
            <li>阅读记录会被统计页读取，用于显示阅读进度、完成状态和总耗时。</li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('window-reader')" alt="阅读器窗口" />
            </div>
            <div class="figure-caption">图 4：阅读器与笔记面板</div>
          </div>
        </section>

        <n-divider />

        <section id="chat">
          <h2 class="h2">聊天与模型</h2>
          <p class="p">
            聊天页由会话列表、中央消息区和右侧上下文区域组成。它适合做资料问答、写作辅助、翻译、总结和把结果回流到编辑器。
          </p>
          <ul class="manual-ul">
            <li>会话列表支持搜索、新建、切换、重命名、保存对话、导出会话和删除会话。</li>
            <li>中央消息区支持复制、分享、重新生成、多选消息、全选可见消息，并可把选中消息发送到编辑器。</li>
            <li>输入区支持附件、翻译模式和模型选择；本地 DeepSeek / Qwen 会在切换时尝试启动对应本地服务。</li>
            <li>模型来源来自设置页：ChatGPT、DeepSeek、Qwen、Kimi、豆包、硅基流动、自定义模型和本地模型。</li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-chat')" alt="聊天页面" />
            </div>
            <div class="figure-caption">图 5：聊天页面</div>
          </div>
        </section>

        <n-divider />

        <section id="brain">
          <h2 class="h2">知识图谱</h2>
          <p class="p">
            知识图谱页用于从知识库文件结构、段落、章节和概念中构建可视化网络。它既能浏览知识库，也能帮助发现节点之间的关系。
          </p>
          <ul class="manual-ul">
            <li>左侧显示知识库与文件树，点击文件可在图谱中定位对应节点。</li>
            <li>3D 图谱支持目录、文件、章节、段落、概念等节点类型过滤，也支持标签多选过滤。</li>
            <li>工具栏支持构建索引、生成向量、AI 提炼概念与关系、语义搜索、路径模式、社区检测/折叠和演化时间轴。</li>
            <li>选中节点后可查看标签、关系、关联建议和标签建议，确认建议后写入图谱；也可以删除手动标签或非结构性关系。</li>
            <li>右侧面板提供节点信息、搜索节点、类型筛选、标签筛选和结果列表。</li>
            <li>
              顶栏的「大脑 / 图谱」开关用来切换同一个页面的两种用法：图谱是你主动去翻的地图，大脑是它替你读完之后主动递给你的结论，详见
              <button type="button" class="toc-link" @click="scrollToSection('cognition')">大脑（认知层）</button>。
            </li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-brain')" alt="知识图谱页面" />
            </div>
            <div class="figure-caption">图 6：知识图谱</div>
          </div>
        </section>

        <n-divider />

        <section id="cognition">
          <h2 class="h2">大脑（认知层）</h2>
          <p class="p">
            大脑是知识图谱页顶栏「大脑 / 图谱」开关切过去的另一半，也是这一版新增的最大一块功能。
            它要解决的问题很具体：材料越攒越多，你其实读不完，也记不住自己三个月前得出过什么结论。
            图谱只能等你想起来去翻，大脑则反过来——它在后台把你已经拥有的材料读一遍，攒成可以被推翻的结论，
            发现其中的矛盾和缺口，然后只在够重要的时候才来打断你一次。
          </p>
          <p class="p">
            有两件事需要先说清楚，否则它看起来会像个不知道在干什么的东西：第一，它不去互联网上替你找新知识，
            只处理你自己的知识库、资料库和知识源抓回来的内容；第二，它默认很安静，
            大多数时候你会看到“没有待处理的提醒”，那是正常状态，不是它没在工作。
          </p>

          <h3 class="h3">四步循环：感知 → 记忆 → 审议 → 投递</h3>
          <ol class="ol">
            <li>
              <b>感知</b>：新抓取的网页、RSS、摘录、文件、聊天记录会进入“收件箱”，每条用本地向量模型算出
              新颖度（和你已有的材料有多不一样）与显著性（值不值得占用你的注意力）。这一步全程本地，不花 token。
            </li>
            <li>
              <b>记忆</b>：从材料里抽出“主张”——一句可以被后来的材料反驳的结论，并记住它出自哪里。
              同时每次问答、抓取、整理都会留下一段行为轨迹，用来统计召回给出的引证里有多少真正被答案用上了。
            </li>
            <li>
              <b>审议</b>：定期扫一遍新感知与图谱变化，产出六类提醒——矛盾、缺口、到期、复习、新兴主题、陈旧。
              每类提醒在界面上都有自己的图标和颜色，悬停能看到这一类到底在说什么。
            </li>
            <li>
              <b>投递</b>：每条提醒按分数和当天剩余额度决定怎么找你——直接打断、系统通知、留在看板，
              或者降级进简报。够不上门槛的提醒不会被丢掉，只是不打断你。
            </li>
          </ol>
          <h3 class="h3">收件箱怎么用</h3>
          <p class="p">
            收件箱是“待读”而不是“已存”。它的目标是让你的阅读清单变短，所以有两条规则最好先知道：
            每天自动采集有条数上限，超出额度的抓取内容根本不会入库；没读过的抓取与 RSS 内容会在设定的天数后自动清除。
            被引证过、被主张或提醒引用过的条目不会被清掉。
          </p>
          <ul class="manual-ul">
            <li>每张卡片有四个图标动作：打开原文、查看全文、归档、不感兴趣。悬停会说明各自的后果。</li>
            <li><b>归档</b>是“留着但不再算未处理”，归档过的条目不会被每日清理带走。</li>
            <li><b>不感兴趣</b>把它从收件箱移出，同时告诉打分器这类内容对你没用，后续同类会被压低。</li>
            <li>左栏可以按来源筛选（网页抓取、RSS、摘录、文件、剪贴板、屏幕、手动、聊天），并看到今日采集额度用掉了多少。</li>
          </ul>

          <h3 class="h3">提醒与打断额度</h3>
          <p class="p">
            打断是有代价的，所以每天能打断你的次数有上限，用完就只剩通知和看板。
            打断门槛还会自适应：你连着采纳提醒，门槛会降下来；你连着忽略，门槛会自己升上去。
            左栏底部一直显示“今日已打断 N 次，还剩 M 次额度”，你随时能看出它现在有多吵。
          </p>
          <ul class="manual-ul">
            <li><b>采纳</b>表示这条提醒有用，同类提醒之后更容易再次找到你。</li>
            <li><b>忽略</b>表示这次打扰错了，同类会被压制。请如实点，门槛就是靠这两个动作学出来的。</li>
            <li>复习类提醒还多一个<b>“忘了，明天再来”</b>，用于推迟而不是否决它。</li>
            <li>简报把当天所有够不上打断门槛的内容汇总成一份，在画像里设定的时刻自动生成；左栏用红色角标提示有没读过的简报。</li>
          </ul>
          <h3 class="h3">画像文件是唯一的真相</h3>
          <p class="p">
            大脑关心什么、什么时候生成简报、每天允许打断几次，全部写在一个可以直接编辑的画像文件里。
            设置页里的表单只是这个文件的编辑器，不是另一份配置——你手工改完画像文件，大脑立刻按新的来。
            这样设计的理由是：你应该能一眼看完并改掉它对你的所有假设，而不是去猜某个开关背后到底是什么。
          </p>

          <h3 class="h3">隐私与安全边界</h3>
          <ul class="manual-ul">
            <li>剪贴板采集和屏幕采集<b>默认关闭</b>，需要你到设置里逐个打开。</li>
            <li>所有外部内容只当资料看，不当指令看。抓回来的正文会先做一次注入中和，卡片上会标出“已中和”。</li>
            <li>向量、新颖度、显著性、矛盾检测全部走本地模型；只有生成简报和少数审议步骤会调用大模型。</li>
            <li>工具授权按读取、写入、联网、执行四档分别授权，并且可以随时查看调用审计。</li>
          </ul>

          <h3 class="h3">先做这三件事</h3>
          <ol class="ol">
            <li>
              在图谱视图里给知识库<b>生成一次向量</b>。召回、新颖度和矛盾检测都依赖它，
              没有向量大脑几乎什么都做不了，左栏会给出橙色提示。
            </li>
            <li>打开设置页的“认知层”，只开你需要的感知来源，先把剪贴板和屏幕留在关闭状态。</li>
            <li>回到大脑视图点一次<b>立即审议</b>，看它产出什么。之后它会自己在后台跑，不需要你再手动点。</li>
          </ol>

          <div class="figure">
            <div class="figure-box">
              <img
                v-if="!isFigurePending('page-cognition')"
                class="figure-img"
                :src="manualImage('page-cognition')"
                alt="大脑（认知层）视图"
                @error="markFigurePending('page-cognition')"
              />
              <div v-else class="figure-pending">
                <div class="figure-pending-title">截图待补充</div>
                <div class="figure-pending-path">{{ figurePath('page-cognition') }}</div>
              </div>
            </div>
            <div class="figure-caption">图 7：大脑（认知层）——左栏、待处理提醒与收件箱</div>
          </div>
        </section>

        <n-divider />

        <section id="search">
          <h2 class="h2">知识源与收集</h2>
          <p class="p">
            知识源页面用于订阅、抓取、阅读和收藏外部内容。它包含分类侧栏、顶部主题搜索、文章列表/杂志流、右侧阅读区和“我的收集”面板。
          </p>
          <ul class="manual-ul">
            <li>顶部搜索可按主题获取知识内容，并支持网格、列表、杂志三种视图切换。</li>
            <li>可开启/关闭后台抓取；需要登录的网站会进入待登录队列，可打开登录或忽略。</li>
            <li>知识源管理窗口支持添加 RSS/Atom、整站深度抓取、Spider 网页探索、知乎、B 站、Quora、小红书、微信公众号、博客/专栏、论坛/社区等来源。</li>
            <li>来源配置可设置归属分类、抓取频率、包含/排除关键字、兴趣描述、相关度阈值和 LLM 增强。</li>
            <li>收藏和摘录会进入“我的收集”，支持搜索、复制 Markdown、插入首页编辑器和删除。</li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-flipboard')" alt="知识源页面" />
            </div>
            <div class="figure-caption">图 8：知识源聚合与阅读</div>
          </div>
        </section>

        <n-divider />

        <section id="task">
          <h2 class="h2">任务管理</h2>
          <p class="p">
            任务管理既有首页文件树中的任务目录，也有独立任务管理窗口。独立窗口以工作、生活、学习三类组织任务，适合日常计划、Bug 跟踪和复盘导出。
          </p>
          <ul class="manual-ul">
            <li>看板视图按待办、进行中、暂停、已完成展示任务，可拖拽变更状态。</li>
            <li>日历视图按截止日期或创建日期展示任务，悬停可查看状态、分类、类型、优先级和时间信息。</li>
            <li>统计视图显示任务总数、已完成、进行中、完成率和 Bug 数，并用图表展示状态、分类完成情况和类型分布。</li>
            <li>支持从 Org 文本、JSON 或文件导入任务；支持生成日报/周报等纯文本报表。</li>
            <li>按字段筛选导出可按分类、类型、状态、严重度、优先级、创建/完成时间和关键字过滤，再选择字段输出。</li>
            <li>设置页可配置任务库根目录、初始化目录、扫描索引、配置远程仓库、提交推送和拉取。</li>
          </ul>
        </section>

        <n-divider />

        <section id="apps">
          <h2 class="h2">应用中心与独立工具</h2>
          <p class="p">
            应用中心按生活、音乐、语言、工具、工作等分类展示应用卡片，点击后以独立窗口打开。部分工具也可以从首页顶部工具栏直接打开。
          </p>
          <ul class="manual-ul">
            <li>相册：管理和浏览本地照片、截图，可按时间、地点等信息组织浏览。</li>
            <li>TO DO：类似 Microsoft To Do 的清单管理，支持列表、分组、任务、步骤、到期日、重复、重要标记和提醒轮询。</li>
            <li>音乐播放器：播放本地或在线音乐，支持播放列表和收藏。</li>
            <li>翻译助手：多语言互译与单词收藏，配合语言学习记录使用。</li>
            <li>文件传输助手：接收服务、局域网发现、HTTP 直连、WebRTC 房间码、任务暂停/继续/取消、目录 HTTP 分享。</li>
            <li>Python Playground：运行和调试 Python 脚本，可用于自动化、数据分析和智能处理。</li>
            <li>有声书 / 配音工坊：导入 PDF、Word、Markdown、TXT、EPUB，清洗文本并合成语音，支持队列、暂停、停止和音频播放。</li>
            <li>模板管理：维护 Markdown 模板，支持默认模板、新建、导入、编辑、删除和插入到编辑器。</li>
            <li>PPT 制作与管理：基于 Slidev 管理演示源文件，支持目录树、轻量编辑器、预览服务、导出 PPTX 和从知识库材料插入卡片。</li>
            <li>数据实验室 / 数学工作区：围绕 Python 变量、表格、结构视图、分析方法、图表和报告生成展开，适合数据分析和数学笔记。</li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-application')" alt="应用中心" />
            </div>
            <div class="figure-caption">图 9：应用中心</div>
          </div>
        </section>

        <n-divider />

        <section id="statistics-whiteboard">
          <h2 class="h2">统计与白板</h2>
          <h3 class="h3">统计页</h3>
          <ul class="manual-ul">
            <li>知识量趋势：按知识点文件数或目录数统计，可选择最近 7 天、30 天、1 年、5 年、10 年、20 年，并按天/月/年聚合。</li>
            <li>活跃热力图：统计 KnowledgeBases 所在 Git 仓库的每日 commit 数，并展示总活跃与活跃天数。</li>
            <li>阅读进度：扫描阅读器 localStorage 记录，展示文档总数、已完成数量、总耗时和单文档进度。</li>
            <li>资料库文档：按知识库筛选导入资料，并展示标签分布。</li>
          </ul>

          <h3 class="h3">白板</h3>
          <p class="p">
            白板页基于 Excalidraw 组件，用于草图、结构图、流程图和临时构思。它与其他主页面共用左侧导航入口，适合把阅读和写作过程中产生的图形想法独立沉淀。
          </p>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-statistics')" alt="统计页面" />
            </div>
            <div class="figure-caption">图 10：统计页面</div>
          </div>
        </section>

        <n-divider />

        <section id="setting">
          <h2 class="h2">设置</h2>
          <p class="p">
            设置页左侧菜单分为内容与库、外观与体验、AI 与模型、系统与服务、帮助与关于。建议首次配置后再开始大量导入资料。
          </p>

          <h3 class="h3">笔记库与资源库</h3>
          <ul class="manual-ul">
            <li>配置知识库根目录、资源库根目录、模板库路径、Slidev PPT 源文件目录和知识库远程仓库信息。</li>
            <li>资源库同步支持不启用、P2P 设备同步、坚果云 WebDAV、Syncthing API。</li>
            <li>P2P 同步支持本机名称、Ticket 有效期、自动同步资源库、同步间隔、并发传输、连接超时、块级增量、自动组网、中继模式、可信设备和配对 Ticket。</li>
          </ul>

          <h3 class="h3">外观与体验</h3>
          <ul class="manual-ul">
            <li>外观主题支持亮色/暗色。</li>
            <li>语音能力可配置 TTS/ASR Provider、模型路径、tokens 路径、exe 路径，并测试文本转语音或音频转写。</li>
            <li>编辑器可启用 LSP 扩展能力；终端可设置默认终端和 Git Bash 路径；文件传输可配置 WebRTC 信令服务地址；系统维护可设置 GC 周期。</li>
          </ul>

          <h3 class="h3">大模型</h3>
          <ul class="manual-ul">
            <li>支持 ChatGPT、DeepSeek、Qwen、Kimi、豆包、硅基流动和自定义模型配置。</li>
            <li>可设置 ChatRecord 保存路径。</li>
            <li>Markdown 转换模型安装到软件同级目录下的 <b>utils/Models/surya_models</b>；转换运行环境安装到 <b>utils/MarkdownConvertor</b>。</li>
            <li>LocalDeepseek / LocalQwen 可下载或从本地文件导入，并在聊天模型切换时启动本地 llama-server 服务。</li>
          </ul>

          <h3 class="h3">任务管理设置</h3>
          <ul class="manual-ul">
            <li>配置任务库根目录，初始化工作/生活/学习目录，立即扫描索引。</li>
            <li>配置任务库 Git 远程地址、用户名和 Email，并可直接提交推送或拉取。</li>
          </ul>

          <h3 class="h3">认知层</h3>
          <p class="p">
            这一页是<button type="button" class="toc-link" @click="scrollToSection('cognition')">大脑（认知层）</button>的开关面板，
            分成感知来源、打扰与预算、工具授权、MCP 接入、我的画像、自调与评测闩、存储与维护七组。
            每个按钮都带悬浮说明，动手之前先看一眼它会改什么。
          </p>
          <ul class="manual-ul">
            <li><b>感知来源</b>：逐个开关网页抓取、RSS、摘录、文件、剪贴板、屏幕、手动、聊天，并设定每天自动采集的条数上限。剪贴板与屏幕默认关闭。</li>
            <li><b>打扰与预算</b>：每天允许打断几次、打断门槛、未读抓取内容保留几天。想让它更安静就把额度调小，提醒不会消失，只会降级进简报。</li>
            <li><b>工具授权</b>：读取、写入、联网、执行四档分别授权，可查看调用审计，确认它到底做过什么。</li>
            <li><b>MCP 接入</b>：把大脑作为 MCP 服务暴露给外部客户端，可以复制配置或读取接入信息。</li>
            <li><b>我的画像</b>：直接读取和编辑画像文件。它是唯一的真相，表单只是它的编辑器。</li>
            <li><b>自调与评测闩</b>：挖掘评测样本、跑一次评测、自动调参、回滚权重。调参必须先过评测这一关，效果变差不会被写进去。</li>
            <li><b>存储与维护</b>：立即清理未读抓取内容、立即打分待处理条目、立即维护、按文件重建画像派生层。</li>
          </ul>

          <div class="figure">
            <div class="figure-box">
              <img
                v-if="!isFigurePending('setting-cognition')"
                class="figure-img"
                :src="manualImage('setting-cognition')"
                alt="设置-认知层"
                @error="markFigurePending('setting-cognition')"
              />
              <div v-else class="figure-pending">
                <div class="figure-pending-title">截图待补充</div>
                <div class="figure-pending-path">{{ figurePath('setting-cognition') }}</div>
              </div>
            </div>
            <div class="figure-caption">图 11：设置页的认知层配置</div>
          </div>

          <div class="figure">
            <div class="figure-box">
              <img class="figure-img" :src="manualImage('page-setting')" alt="设置页面" />
            </div>
            <div class="figure-caption">图 12：设置页面</div>
          </div>
        </section>

        <n-divider />

        <section id="language-learning">
          <h2 class="h2">语言学习</h2>
          <p class="p">
            语言学习窗口汇总单词、例句、段落和听力资源。它既能从阅读器选中文本沉淀词汇，也能管理本地或在线听力材料。
          </p>
          <ul class="manual-ul">
            <li>词汇管理：搜索单词、释义、例句、笔记；新增/编辑/删除学习记录；刷新单词拓展信息，包括音标、释义、近义词、反义词、关联词和例句。</li>
            <li>随机抽查：支持抽卡、翻牌、上一组/下一组、单词与听力混合卡片。</li>
            <li>听力管理：导入本地音频，按主题采集听力，配置听力源，筛选资源类型、Provider 和中文分类。</li>
            <li>沉浸听力：播放音频/视频，显示原文/译文/双语字幕，支持字体缩放、字幕模式切换和同步时间轴。</li>
            <li>复习训练：单词、听力或混合复习，可按创建时间、上次复习时间、掌握度和排序方式筛选。</li>
            <li>导入导出：可导出学习数据包，也可导入恢复，听力资源会按范围处理资产包。</li>
          </ul>
        </section>

        <n-divider />

        <section id="faq">
          <h2 class="h2">常见问题</h2>
          <div class="qa">
            <div class="q">Q：首页搜索和知识源搜索有什么区别？</div>
            <div class="a">A：首页搜索面向本地知识库/目录/摘录；知识源搜索面向外部内容抓取、订阅和阅读。</div>
          </div>
          <div class="qa">
            <div class="q">Q：Markdown 转换模型安装后仍不可用？</div>
            <div class="a">A：检查 <b>utils/Models/surya_models</b> 和 <b>utils/MarkdownConvertor</b> 是否存在；安装或导入后建议重启应用再尝试转换。</div>
          </div>
          <div class="qa">
            <div class="q">Q：Git 推送/拉取失败怎么办？</div>
            <div class="a">A：知识库和任务库都直接调用本机 git 命令。请先在系统终端中确认对应目录能正常 git pull / git push，再回到 Guaxi 操作。</div>
          </div>
          <div class="qa">
            <div class="q">Q：本地模型为什么切换后要等一会儿？</div>
            <div class="a">A：LocalDeepseek / LocalQwen 会在选择时检查并启动本地服务，首次加载模型需要等待模型文件和 llama-server 初始化。</div>
          </div>
          <div class="qa">
            <div class="q">Q：大脑一直显示“没有待处理的提醒”，是不是坏了？</div>
            <div class="a">A：大多数时候这就是正常状态，它只在够重要时才打断你。如果连简报也是空的，先确认知识库已经<b>生成过向量</b>——左栏会用橙色提示语义服务未就绪，那种情况下召回、新颖度和矛盾检测都跑不起来。</div>
          </div>
          <div class="qa">
            <div class="q">Q：收件箱里的抓取内容为什么自己消失了？</div>
            <div class="a">A：这是有意的。没读过的抓取与 RSS 内容会在设定的天数后自动清除，每天自动采集也有条数上限，超出的根本不入库。想留下某一条，把它<b>归档</b>，或者在任何地方引用它一次；被引证、被主张或提醒引用过的条目不会被清。</div>
          </div>
          <div class="qa">
            <div class="q">Q：提醒太吵或者太少，怎么调？</div>
            <div class="a">A：如实点“采纳”和“忽略”，打断门槛会自己跟着走：连着采纳就降下来，连着忽略就升上去。想立刻见效，就去设置页“认知层 → 打扰与预算”改每天打断次数和门槛，或者直接编辑画像文件。</div>
          </div>
          <div class="qa">
            <div class="q">Q：大脑会读我的剪贴板和屏幕吗？</div>
            <div class="a">A：默认不会，这两个来源出厂就是关闭的，要你自己去设置页打开。其他来源抓回来的正文会先做注入中和再入库，外部内容只当资料看、不当指令执行。</div>
          </div>
        </section>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { computed, onMounted, reactive } from 'vue'
import { NAlert, NCard, NDivider, NSpace, NTag } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiManual')
const settingsStore = useSettingsStore()

const isDarkTheme = computed(() => {
  const storeTheme = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  if (storeTheme === 'dark') return true
  if (storeTheme === 'light') return false
  const dataTheme = document?.documentElement?.dataset?.guaxiTheme
  return dataTheme === 'dark'
})

// 设计思路：手册截图统一走 light/dark 双版本映射，正文更新时不改截图调用方式。
// 这样后续只需要替换 assets/manual 下的图片，不需要再改文档结构。
const manualImages = {
  overview: {
    light: new URL('../../assets/manual/overview.png', import.meta.url).href,
    dark: new URL('../../assets/manual/overview-dark.png', import.meta.url).href
  },
  'setting-noterepo': {
    light: new URL('../../assets/manual/setting-noterepo.png', import.meta.url).href,
    dark: new URL('../../assets/manual/setting-noterepo-dark.png', import.meta.url).href
  },
  'page-home': {
    light: new URL('../../assets/manual/page-home.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-home-dark.png', import.meta.url).href
  },
  'window-reader': {
    light: new URL('../../assets/manual/window-reader.png', import.meta.url).href,
    dark: new URL('../../assets/manual/window-reader-dark.png', import.meta.url).href
  },
  'page-chat': {
    light: new URL('../../assets/manual/page-chat.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-chat-dark.png', import.meta.url).href
  },
  'page-brain': {
    light: new URL('../../assets/manual/page-brain.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-brain-dark.png', import.meta.url).href
  },
  // 认知层这两张截图还没补。文件放进 assets/manual 就会自动显示出来，
  // 在那之前正文里显示的是一个写着目标路径的占位框，不是坏图标。
  'page-cognition': {
    light: new URL('../../assets/manual/page-cognition.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-cognition-dark.png', import.meta.url).href
  },
  'setting-cognition': {
    light: new URL('../../assets/manual/setting-cognition.png', import.meta.url).href,
    dark: new URL('../../assets/manual/setting-cognition-dark.png', import.meta.url).href
  },
  'page-flipboard': {
    light: new URL('../../assets/manual/page-flipboard.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-flipboard-dark.png', import.meta.url).href
  },
  'page-application': {
    light: new URL('../../assets/manual/page-application.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-application-dark.png', import.meta.url).href
  },
  'page-statistics': {
    light: new URL('../../assets/manual/page-statistics.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-statistics-dark.png', import.meta.url).href
  },
  'page-setting': {
    light: new URL('../../assets/manual/page-setting.png', import.meta.url).href,
    dark: new URL('../../assets/manual/page-setting-dark.png', import.meta.url).href
  }
}

const manualImage = (key) => {
  const entry = manualImages[String(key || '')]
  if (!entry) return ''
  return isDarkTheme.value ? entry.dark : entry.light
}

// 截图还没放进 assets/manual 时，浏览器会给出一个坏图标，读手册的人只会以为软件出错了。
// 这里记下加载失败的图，改成显示目标路径的占位框；亮色和暗色分别记，
// 因为经常是先补一张、另一张还欠着。图片补齐后占位框自动消失，正文不用再改。
const brokenFigures = reactive(new Set())
const figureSlot = (key) => `${key}:${isDarkTheme.value ? 'dark' : 'light'}`
const isFigurePending = (key) => brokenFigures.has(figureSlot(key))
const markFigurePending = (key) => {
  brokenFigures.add(figureSlot(key))
  logger.warn('[Manual] figure missing', { key, dark: isDarkTheme.value })
}
const figurePath = (key) =>
  `src/assets/manual/${key}${isDarkTheme.value ? '-dark' : ''}.png`

// 关键交互日志：手册是设置页内的长文档，记录跳转行为能帮助排查目录锚点失效或主题切换后的布局问题。
const scrollToSection = (id) => {
  const targetId = String(id || '')
  const el = document.getElementById(targetId)
  if (!el) {
    logger.warn('[Manual] section not found', { targetId })
    return
  }
  logger.info('[Manual] scroll to section', { targetId })
  el.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

onMounted(() => {
  logger.info('[Manual] mounted current manual', {
    sections: Object.keys(manualImages).length,
    theme: isDarkTheme.value ? 'dark' : 'light'
  })
})
</script>

<style scoped>
.manual {
  width: 100%;
  box-sizing: border-box;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
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
  line-height: 1.9;
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

.h2 {
  margin: 0;
  font-size: 18px;
  line-height: 1.4;
}

.h3 {
  margin: 14px 0 6px;
  font-size: 15px;
  line-height: 1.4;
}

.p {
  margin: 8px 0 0;
  color: var(--guaxi-text-primary);
  line-height: 1.8;
}

.ol {
  margin: 8px 0 0;
  padding-left: 20px;
  line-height: 1.9;
}

.manual-ul {
  margin: 8px 0 0;
  padding-left: 20px;
  line-height: 1.9;
}

.manual-ul li,
.ol li {
  margin: 2px 0;
}

.feature-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 10px;
}

.feature-card {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 12px;
  background: var(--guaxi-panel-bg);
  appearance: none;
  cursor: pointer;
  text-align: left;
  width: 100%;
  min-height: 112px;
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

.figure {
  margin-top: 12px;
}

.figure-box {
  border: 1px dashed var(--guaxi-border);
  border-radius: 8px;
  min-height: 360px;
  padding: 0;
  display: flex;
  flex-direction: column;
  justify-content: stretch;
  align-items: stretch;
  background: rgba(127, 127, 127, 0.08);
  user-select: none;
  overflow: hidden;
}

/* 截图还没放进来时显示的占位框。虚线边框和最小高度沿用 .figure-box，
   所以补图前后正文的排版不会跳动，读者也知道这里本来该有一张图。 */
.figure-pending {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 24px 16px;
  color: var(--guaxi-text-secondary);
}

.figure-pending-title {
  font-size: 13px;
}

.figure-pending-path {
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 12px;
  word-break: break-all;
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
:root[data-guaxi-theme='dark'] .manual .feature-title,
:root[data-guaxi-theme='dark'] .manual .toc-title,
:root[data-guaxi-theme='dark'] .manual .q {
  color: #f0f6fc;
}

:root[data-guaxi-theme='dark'] .manual .p,
:root[data-guaxi-theme='dark'] .manual .feature-desc,
:root[data-guaxi-theme='dark'] .manual .manual-ul,
:root[data-guaxi-theme='dark'] .manual .ol,
:root[data-guaxi-theme='dark'] .manual .a,
:root[data-guaxi-theme='dark'] .manual .figure-pending,
:root[data-guaxi-theme='dark'] .manual .figure-caption {
  color: #c9d1d9;
}

.dark-mode .manual .h2,
.dark-mode .manual .h3,
.dark-mode .manual .feature-title,
.dark-mode .manual .toc-title,
.dark-mode .manual .q {
  color: #f0f6fc;
}

.dark-mode .manual .p,
.dark-mode .manual .feature-desc,
.dark-mode .manual .manual-ul,
.dark-mode .manual .ol,
.dark-mode .manual .a,
.dark-mode .manual .figure-pending,
.dark-mode .manual .figure-caption {
  color: #c9d1d9;
}

@media (max-width: 900px) {
  .feature-grid {
    grid-template-columns: 1fr;
  }

  .figure-box {
    min-height: 240px;
  }
}
</style>
