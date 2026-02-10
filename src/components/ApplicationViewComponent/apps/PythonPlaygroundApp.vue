<template>
  <div class="py-root">
    <n-card title="Python Playground" size="small" class="py-card">
      <n-space vertical :size="10">
        <n-alert type="info" :bordered="false">
          约定：在脚本里设置 <span class="mono">_result</span> 作为返回值；打印内容会出现在 stdout/stderr。
        </n-alert>

        <n-space align="center" justify="space-between">
          <n-space align="center">
            <n-switch v-model:value="persist">
              <template #checked>REPL 开</template>
              <template #unchecked>REPL 关</template>
            </n-switch>

            <n-button size="small" @click="handleResetRepl" :disabled="running">
              重置 REPL
            </n-button>
          </n-space>

          <n-space align="center">
            <n-select size="small" style="width: 260px" v-model:value="sampleKey" :options="sampleOptions"
              :disabled="running" />
            <n-button size="small" @click="applySample" :disabled="running">插入示例</n-button>
          </n-space>
        </n-space>

        <n-space align="center">
          <n-input v-model:value="selectedFilePath" placeholder="可选：选择一个文本/CSV/JSON 文件，把内容注入到 _input_text" clearable />
          <n-button size="small" @click="pickFile" :disabled="running">选择文件</n-button>
          <n-button size="small" @click="loadFileToVars" :disabled="running || !selectedFilePath">
            注入 _input_text
          </n-button>
        </n-space>

        <div class="grid">
          <div class="col">
            <div class="label">Python 代码</div>
            <n-input v-model:value="code" type="textarea" :autosize="{ minRows: 14, maxRows: 28 }"
              placeholder="写 Python 代码，运行后查看 stdout/stderr/_result" />
            <n-space justify="end" style="margin-top: 10px">
              <n-button type="primary" @click="run" :loading="running">运行</n-button>
            </n-space>
          </div>

          <div class="col">
            <div class="label">stdout</div>
            <n-input v-model:value="stdout" type="textarea" readonly :autosize="{ minRows: 6, maxRows: 10 }" />
            <div class="label" style="margin-top: 10px">stderr</div>
            <n-input v-model:value="stderr" type="textarea" readonly :autosize="{ minRows: 6, maxRows: 10 }" />
            <div class="label" style="margin-top: 10px">_result（JSON）</div>
            <n-input v-model:value="resultJsonText" type="textarea" readonly :autosize="{ minRows: 6, maxRows: 10 }" />
          </div>
        </div>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useMessage, NCard, NInput, NButton, NSpace, NSelect, NSwitch, NAlert } from 'naive-ui'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('PythonPlaygroundApp')
const message = useMessage()

const running = ref(false)
const persist = ref(true)

const selectedFilePath = ref('')
const injectedTextLen = ref(0)

const code = ref('')
const stdout = ref('')
const stderr = ref('')
const resultJson = ref(null)

const resultJsonText = computed(() => {
  try {
    if (resultJson.value == null) return ''
    return JSON.stringify(resultJson.value, null, 2)
  } catch {
    return String(resultJson.value ?? '')
  }
})

const sampleKey = ref('hello')
const sampleOptions = [
  { label: 'Hello / 变量回传', value: 'hello' },
  { label: '数据概览（JSON/CSV）', value: 'profile' },
  { label: '简易智能标签（关键词）', value: 'tags' },
]

const SAMPLES = {
  hello: `print("Hello from Python!")\nname = "Guaxi"\n_result = {"hello": name, "len": len(name)}\n`,
  profile: `import json, csv, io\n\ntext = _input_text if isinstance(_input_text, str) else ""\ntext = text.strip()\n\ndef profile_json(obj):\n  if isinstance(obj, dict):\n    return {\n      "kind": "object",\n      "keys": list(obj.keys())[:50],\n      "size": len(obj),\n    }\n  if isinstance(obj, list):\n    return {\n      "kind": "array",\n      "size": len(obj),\n      "head": obj[:5],\n    }\n  return {\"kind\": type(obj).__name__}\n\ndef profile_csv(text):\n  f = io.StringIO(text)\n  reader = csv.reader(f)\n  rows = list(reader)\n  if not rows:\n    return {\"kind\": \"csv\", \"rows\": 0, \"cols\": 0}\n  header = rows[0]\n  data = rows[1:]\n  return {\n    \"kind\": \"csv\",\n    \"rows\": len(data),\n    \"cols\": len(header),\n    \"header\": header,\n    \"preview\": data[:5]\n  }\n\nout = {\"input_len\": len(text)}\n\nif not text:\n  out[\"error\"] = \"请先注入 _input_text（右上方选择文件 → 注入 _input_text）\"\nelse:\n  # 尝试 JSON\n  try:\n    obj = json.loads(text)\n    out[\"format\"] = \"json\"\n    out[\"profile\"] = profile_json(obj)\n  except Exception:\n    # 尝试 CSV\n    try:\n      out[\"format\"] = \"csv\"\n      out[\"profile\"] = profile_csv(text)\n    except Exception as e:\n      out[\"format\"] = \"text\"\n      out[\"error\"] = str(e)\n\nprint(\"profile done, format=\", out.get(\"format\"))\n_result = out\n`,
  tags: `import re\n\ntext = _input_text if isinstance(_input_text, str) else \"\"\ntext = text.lower()\nwords = re.findall(r\"[a-zA-Z0-9_\\u4e00-\\u9fa5]{2,}\", text)\n\nstop = set([\"the\",\"and\",\"for\",\"with\",\"this\",\"that\",\"from\",\"http\",\"https\",\"www\"]) \n\nfreq = {}\nfor w in words:\n  if w in stop:\n    continue\n  freq[w] = freq.get(w, 0) + 1\n\nitems = sorted(freq.items(), key=lambda x: x[1], reverse=True)\ntop = items[:20]\n\n_result = {\n  \"top_keywords\": [{\"w\": w, \"c\": c} for w, c in top],\n  \"suggested_tags\": [w for w, _ in top[:10]]\n}\nprint(\"tags generated:\", \", \".join(_result[\"suggested_tags\"]))\n`,
}

function applySample() {
  const k = sampleKey.value
  code.value = SAMPLES[k] || ''
  logger.info('[Python] apply sample', { k })
}

applySample()

async function pickFile() {
  try {
    const filePath = await open({
      multiple: false,
      filters: [{ name: 'Text/CSV/JSON', extensions: ['txt', 'md', 'log', 'csv', 'json'] }],
    })
    if (!filePath) return
    selectedFilePath.value = String(filePath)
    logger.info('[Python] pick file', { path: selectedFilePath.value })
  } catch (e) {
    logger.error('[Python] pick file failed', e)
    message.error('选择文件失败')
  }
}

async function loadFileToVars() {
  try {
    if (!selectedFilePath.value) return
    const text = await invoke('read_file_content', { path: selectedFilePath.value })
    injectedTextLen.value = (text || '').length
    await injectVars({ _input_text: text, _input_path: selectedFilePath.value })
    message.success(`已注入 _input_text（${injectedTextLen.value} 字符）`)
  } catch (e) {
    logger.error('[Python] load file failed', e)
    message.error('注入 _input_text 失败')
  }
}

const vars = ref({})

async function injectVars(obj) {
  vars.value = { ...(vars.value || {}), ...(obj || {}) }
}

async function handleResetRepl() {
  try {
    await invoke('python_reset_repl')
    message.success('REPL 已重置')
    logger.info('[Python] repl reset')
  } catch (e) {
    logger.error('[Python] repl reset failed', e)
    message.error('REPL 重置失败')
  }
}

async function run() {
  running.value = true
  stdout.value = ''
  stderr.value = ''
  resultJson.value = null

  try {
    logger.info('[Python] run start', { persist: persist.value, codeLen: (code.value || '').length })
    const res = await invoke('python_exec', {
      request: {
        code: code.value,
        persist: persist.value,
        expect_json: true,
        vars: vars.value,
      },
    })

    stdout.value = res?.stdout || ''
    stderr.value = res?.stderr || ''
    resultJson.value = res?.result_json ?? null

    if (res?.ok) {
      message.success('运行成功')
    } else {
      message.warning('运行结束（有错误）')
    }

    logger.info('[Python] run done', {
      ok: !!res?.ok,
      stdoutLen: (stdout.value || '').length,
      stderrLen: (stderr.value || '').length,
    })
  } catch (e) {
    logger.error('[Python] run invoke failed', e)
    message.error('运行失败（后端调用异常）')
  } finally {
    running.value = false
  }
}
</script>

<style scoped>
.py-root {
  height: 100%;
  min-height: 0;
  padding: 12px;
  box-sizing: border-box;
}

.py-card {
  height: 100%;
  min-height: 0;
}

.grid {
  display: grid;
  grid-template-columns: 1.2fr 1fr;
  gap: 12px;
  min-height: 0;
}

.col {
  min-height: 0;
}

.label {
  font-size: 12px;
  color: var(--guaxi-text-2, #666);
  margin-bottom: 6px;
}

.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
}
</style>