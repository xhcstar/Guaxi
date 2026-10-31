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

test('规则模型：条件之间有与 / 或，每一项支持「不包含」', () => {
  assert.ok(/pub match_mode: String/.test(model), '规则保留 matchMode（all / any）')
  assert.ok(/pub from_not_contains: Vec<String>/.test(model), '发件人不包含')
  assert.ok(/pub to_not_contains: Vec<String>/.test(model), '收件人不包含')
  assert.ok(/pub body_not_contains: Vec<String>/.test(model), '正文不包含')
  assert.ok(
    /pub from_not_contains[\s\S]{0,120}#\[serde\(default\)\]|#\[serde\(default\)\][\s\S]{0,80}pub from_not_contains/.test(model),
    '新字段要有 serde 默认值，老规则照常读',
  )
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
  // 账户 / 文件夹 / 附件 / 大小是「管哪批邮件」的范围，不能被「任一满足」绕过。
  assert.ok(/作用域不是筛选条件/.test(rules), '范围条件单独判定')
  assert.ok(
    /if let Some\(want\) = cond\.has_attachment[\s\S]{0,120}return false;[\s\S]{0,400}let mut terms/.test(rules),
    '附件检查发生在与或之前',
  )
  // 预筛：正文相关项按「可能命中」算，宁多解一次也不漏。
  assert.ok(/预筛阶段/.test(rules), '预筛对正文条件保守处理')
  assert.ok(/!cond\.body_not_contains\.is_empty\(\)/.test(rules), '正文不包含也算依赖正文')
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
  assert.ok(/v-model="draft\.matchMode"/.test(dialog), '模式下拉绑定 matchMode')
  assert.ok(/value="all"[\s\S]{0,40}全部满足（与）/.test(dialog), '与')
  assert.ok(/value="any"[\s\S]{0,40}任一满足（或）/.test(dialog), '或')
  assert.ok(/v-model="cond\.fromNot"/.test(dialog) && /v-model="cond\.toNot"/.test(dialog), '发件人 / 收件人不包含')
  assert.ok(/v-model="cond\.bodyNot"/.test(dialog), '正文不包含')
  assert.ok(/fromNotContains: splitList\(cond\.fromNot\)/.test(dialog), '不包含要落进规则')
  assert.ok(/toNotContains: splitList\(cond\.toNot\)/.test(dialog), '收件人不包含要落进规则')
  assert.ok(/bodyNotContains: splitList\(cond\.bodyNot\)/.test(dialog), '正文不包含要落进规则')
  assert.ok(/rule\.matchMode === 'any' \? ' 或 ' : '，'/.test(dialog), '摘要里能看出是「或」')
  assert.ok(/发件人不含/.test(dialog) && /正文不含/.test(dialog), '摘要列出不含条件')
})
