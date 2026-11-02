import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// 第二十六轮：规则条件支持 Outlook 式的「与 / 或 / 非」。
const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.join(here, '..')
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8')

const model = read('src-tauri/src/commands/mail/model.rs')
const rules = read('src-tauri/src/commands/mail/rules.rs')
const sieve = read('src-tauri/src/commands/mail/sieve.rs')
const dialog = read('src/components/ApplicationViewComponent/apps/mail/MailRulesDialog.vue')
const group = read('src/components/ApplicationViewComponent/apps/mail/MailRuleGroup.vue')

test('规则模型：条件之间有与 / 或，每一项支持「不包含」', () => {
  assert.ok(/pub match_mode: String/.test(model), '规则保留 matchMode（all / any）')
  assert.ok(/pub from_not_contains: Vec<String>/.test(model), '发件人不包含')
  assert.ok(/pub to_not_contains: Vec<String>/.test(model), '收件人不包含')
  assert.ok(/pub body_not_contains: Vec<String>/.test(model), '正文不包含')
  assert.ok(
    /pub from_not_contains[\s\S]{0,120}#\[serde\(default\)\]|#\[serde\(default\)\][\s\S]{0,80}pub from_not_contains/.test(model),
    '新字段要有 serde 默认值，老规则照常读',
  )
  // 新版嵌套条件组：组内 / 组间都能选与或非。
  assert.ok(/pub struct RuleClause/.test(model) && /pub struct RuleGroup/.test(model), '叶子条件与条件组')
  assert.ok(/pub groups: Vec<RuleGroup>/.test(model), '条件组可递归嵌套')
  assert.ok(/pub not: bool/.test(model), '整组可取反')
})

test('匹配引擎：all = 全部满足，any = 任一满足；范围条件不参与「或」', () => {
  assert.ok(/fn is_any_mode/.test(rules), '识别 any / or')
  assert.ok(
    /fn conditions_match\(cond: &RuleCondition, match_mode: &str/.test(rules),
    'match_mode 传进判定',
  )
  assert.ok(/conditions_match\(&rule\.condition, &rule\.match_mode, ctx\)/.test(rules), 'evaluate 用规则自己的模式')
  assert.ok(
    /if is_any_mode\(match_mode\)[\s\S]{0,80}terms\.iter\(\)\.any[\s\S]{0,120}terms\.iter\(\)\.all/.test(rules),
    '与 / 或分别走 all / any',
  )
  // 嵌套条件组同样按 mode 组合，整组可取反。
  assert.ok(/fn group_match/.test(rules) && /is_any_mode\(&group\.mode\)/.test(rules), '组内与或')
  assert.ok(/if group\.not[\s\S]{0,40}!base/.test(rules), '组级取反')
  // 账户 / 文件夹 / 附件 / 大小是「管哪批邮件」的范围，不能被「任一满足」绕过。
  assert.ok(/作用域不是筛选条件/.test(rules), '范围条件单独判定')
  assert.ok(
    /fn scope_matches[\s\S]{0,900}cond\.has_attachment[\s\S]{0,120}return false;/.test(rules),
    '附件检查发生在与或之前',
  )
  assert.ok(
    /fn conditions_match_inner[\s\S]{0,200}scope_matches\(cond, ctx\)/.test(rules),
    '内容判定前先过范围',
  )
  // 预筛：正文相关项按「可能命中」算，宁多解一次也不漏。
  assert.ok(/预筛阶段/.test(rules), '预筛对正文条件保守处理')
  assert.ok(/!cond\.body_not_contains\.is_empty\(\)/.test(rules), '正文不包含也算依赖正文')
  assert.ok(/groups_need_body/.test(rules) && /body_relevant/.test(rules), '条件组里含正文时保守解正文')
})

test('Sieve：服务器端同样按与 / 或生成，且带上「不包含」', () => {
  assert.ok(/anyof \(\{}\)/.test(sieve) && /allof \(\{}\)/.test(sieve), '按模式生成 anyof / allof')
  assert.ok(/rule\.match_mode\.trim\(\)\.to_ascii_lowercase\(\)/.test(sieve), '读规则自己的模式')
  assert.ok(/not address :contains \\"From\\"/.test(sieve), '发件人不包含')
  assert.ok(/not address :contains \[\\"To\\",\\"Cc\\"\]/.test(sieve), '收件人不包含')
  // 附件 / 大小并入范围，和内容条件仍是「与」。
  assert.ok(/parts\.extend\(scope\)/.test(sieve), '范围条件始终为与')
})

test('编辑器：可以选「全部满足 / 任一满足」，并填不包含', () => {
  // 条件树由 MailRuleGroup 递归渲染：组内与或、整组取反、嵌套条件组。
  assert.ok(/v-model="group\.mode"/.test(group), '模式下拉绑定 group.mode')
  assert.ok(/value="all"[\s\S]{0,40}全部满足（且）/.test(group), '且')
  assert.ok(/value="any"[\s\S]{0,40}任一满足（或）/.test(group), '或')
  assert.ok(/v-model="group\.not"/.test(group), '整组取反')
  assert.ok(/v-model="clause\.op"/.test(group), '每条条件选操作符')
  assert.ok(/notContains/.test(group) && /value:\s*'notContains'[\s\S]{0,40}不包含/.test(group), '支持「不包含」')
  assert.ok(/<MailRuleGroup[\s\S]{0,120}:group="child"/.test(group), '条件组可以嵌套')
  // 保存时拍平成后端认的 condition.groups，并保留顶层模式。
  assert.ok(/groups: root \? \[root\] : \[\]/.test(dialog), '条件树写进 groups')
  assert.ok(/matchMode: root\?\.mode \|\| 'all'/.test(dialog), '顶层模式跟着条件组走')
  assert.ok(/matchMode === 'any' \? ' 或 ' : ' 且 '/.test(dialog), '摘要里能看出是「或」')
  assert.ok(/group\.not \? `非 \$\{body\}` : body/.test(dialog), '摘要列出整组取反')
})
