// 简历的数据模型与表单字段定义。
//
// 模块（section）是唯一的文档结构来源：表单、预览、导出、模板都从它渲染，
// 不再各自维护一套 DOM。

let seq = 0
export const uid = (prefix = 'id') =>
  `${prefix}_${Date.now().toString(36)}_${(seq++).toString(36)}${Math.random().toString(36).slice(2, 6)}`

export const SECTION_TYPES = [
  { type: 'education', label: '教育经历', hint: '学校、专业与学历' },
  { type: 'work', label: '工作经历', hint: '公司、岗位与职责' },
  { type: 'project', label: '项目经历', hint: '项目背景、职责与技术点' },
  { type: 'skills', label: '专业技能', hint: '每行一条技能' },
  { type: 'custom', label: '自定义模块', hint: '奖项、开源、证书等' },
]

export const SECTION_META = Object.fromEntries(SECTION_TYPES.map((item) => [item.type, item]))

export const SECTION_FIELDS = {
  education: [
    { key: 'period', label: '时间', placeholder: '2015.09-2019.07', span: 1, mono: true },
    { key: 'school', label: '学校', placeholder: '星河理工大学', span: 1 },
    { key: 'college', label: '学院', placeholder: '电子工程学院', span: 1 },
    { key: 'major', label: '专业', placeholder: '微电子科学与工程', span: 1 },
    { key: 'degree', label: '学历 / 学位', placeholder: '本科', span: 1 },
  ],
  work: [
    { key: 'period', label: '时间', placeholder: '2023.01-至今', span: 1, mono: true },
    { key: 'company', label: '公司', placeholder: '北辰安全科技股份有限公司', span: 1 },
    { key: 'department', label: '部门', placeholder: 'R&D Department', span: 1 },
    { key: 'role', label: '岗位', placeholder: '研发工程师', span: 1 },
    { key: 'description', label: '主要职责 / 业绩', type: 'lines', span: 2, placeholder: '每行一条，例如：负责 XX 模块的设计与交付' },
  ],
  project: [
    { key: 'period', label: '时间', placeholder: '2020.02-2020.05', span: 1, mono: true },
    { key: 'name', label: '项目名称', placeholder: 'WiFi + BLE 智能家居网关设备', span: 1 },
    { key: 'description', label: '项目简介', placeholder: '一句话说明项目做了什么', span: 2 },
    { key: 'details', label: '职责 / 技术点', type: 'lines', span: 2, placeholder: '每行一条，例如：设计设备配网流程和状态机' },
  ],
  custom: [
    { key: 'period', label: '时间', placeholder: '2024.03', span: 1, mono: true },
    { key: 'name', label: '名称', placeholder: '开源贡献 / 获奖名称', span: 1 },
    { key: 'description', label: '说明', placeholder: '一句话说明', span: 2 },
    { key: 'details', label: '要点', type: 'lines', span: 2, placeholder: '每行一条' },
  ],
}

export function createItem(type) {
  if (type === 'skills') return { id: uid('it'), text: '' }
  const item = { id: uid('it') }
  for (const field of SECTION_FIELDS[type] || SECTION_FIELDS.custom) item[field.key] = ''
  return item
}

export function createSection(type, overrides = {}) {
  if (type === 'skills') {
    return { id: uid('sc'), type, title: '专业技能', visible: true, items: [createItem('skills')], ...overrides }
  }
  return {
    id: uid('sc'),
    type,
    title: SECTION_META[type]?.label || '自定义模块',
    visible: true,
    items: [createItem(type)],
    ...overrides,
  }
}

export function itemTitle(type, item) {
  if (type === 'skills') return item.text || '新技能'
  return item[itemTitleKey(type)] || '未填写'
}

export function itemTitleKey(type) {
  if (type === 'work') return 'company'
  if (type === 'education') return 'school'
  return 'name'
}

export function createEmptyDocument(templateId = 'builtin:blueprint') {
  return {
    title: '未命名简历',
    templateId,
    data: {
      schemaVersion: 2,
      profile: {
        name: '',
        title: '',
        city: '',
        phone: '',
        email: '',
        website: '',
        target: '',
        summary: '',
        avatar: '',
      },
      sections: [
        createSection('education', { items: [createItem('education')] }),
        createSection('work', { items: [createItem('work')] }),
        createSection('skills', { items: [createItem('skills')] }),
      ],
    },
  }
}

export function createSampleDocument(templateId = 'builtin:blueprint') {
  return {
    title: '林泽远 · 嵌入式软件工程师',
    templateId,
    data: {
      schemaVersion: 2,
      profile: {
        name: '林泽远',
        title: '嵌入式软件工程师',
        city: '星海市',
        phone: '138-0000-0000',
        email: 'linzy@example.com',
        website: 'github.com/linzy-demo',
        target: 'Linux C 开发 / 嵌入式开发',
        summary: '具备 Linux 网络、嵌入式系统与 IoT 设备软件开发经验，熟悉从需求拆解、模块设计、联调定位到交付维护的完整流程。',
        avatar: '',
      },
      sections: [
        createSection('education', {
          items: [{
            id: uid('it'),
            period: '2015.09-2019.07',
            school: '星河理工大学',
            college: '电子工程学院',
            major: '微电子科学与工程',
            degree: '本科',
          }],
        }),
        createSection('work', {
          items: [
            { id: uid('it'), period: '2019.07-2020.08', company: '蓝海智联科技有限公司', department: '物联网研究院', role: '嵌入式软件工程师', description: '负责物联网设备通信模块的软件开发工作' },
            { id: uid('it'), period: '2020.09-2022.04', company: '云岭开源软件有限公司', department: '操作系统事业部', role: '软件工程师', description: '负责 Euler / OpenEuler 操作系统网络相关软件包开发及维护工作' },
            { id: uid('it'), period: '2023.01-至今', company: '北辰安全科技股份有限公司', department: 'R&D Department', role: '研发工程师', description: '负责 APV 设备 SSL 相关功能的开发和维护工作' },
          ],
        }),
        createSection('skills', {
          items: [
            { id: uid('it'), text: '熟悉 C/C++、Python、Shell 和基本的数据结构与算法' },
            { id: uid('it'), text: '熟悉 Linux 系统下网络 / 启动相关软件包的问题定位' },
            { id: uid('it'), text: '熟悉 TCP/IP、HTTP、TLS 等常见网络协议' },
            { id: uid('it'), text: '熟悉 FreeRTOS 等常见嵌入式操作系统' },
          ],
        }),
        createSection('project', {
          items: [{
            id: uid('it'),
            period: '2019.10-2020.01',
            name: '双云智能家居项目开发',
            description: '实现智能电器与虚拟 IoT 平台对接，支持用户通过移动端 APP 控制智能电器。',
            details: '负责通信模块的软件开发工作\n主要技术：C、FreeRTOS',
          }],
        }),
      ],
    },
  }
}

export function normalizeDocument(raw) {
  const data = raw?.data || {}
  const sections = Array.isArray(data.sections) ? data.sections : []
  return {
    title: raw?.title || '未命名简历',
    templateId: raw?.templateId || 'builtin:blueprint',
    data: {
      schemaVersion: 2,
      profile: {
        name: '',
        title: '',
        city: '',
        phone: '',
        email: '',
        website: '',
        target: '',
        summary: '',
        avatar: '',
        ...(data.profile || {}),
      },
      sections: sections
        .filter((section) => section && section.type)
        .map((section) => ({
          id: section.id || uid('sc'),
          type: section.type,
          title: section.title || SECTION_META[section.type]?.label || '自定义模块',
          visible: section.visible !== false,
          items: Array.isArray(section.items) ? section.items : [],
        })),
    },
  }
}
