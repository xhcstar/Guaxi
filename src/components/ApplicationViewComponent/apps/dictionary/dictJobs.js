import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 词典任务（下载 / 建索引）的状态放在模块级，不挂在组件上。
// 这样管理器关掉、切到别的页面、甚至换个应用再回来，轮询都不会断：
// 后端本来就跑在独立任务里，前端只需要保证「随时能看到进度」。
// 没有任务在跑时自动停表，不会常驻空轮询。

const ACTIVE_STAGES = new Set(['download', 'import'])
const FINISHED_STAGES = new Set(['done', 'error'])

export const dictJobs = reactive({
  map: {},
  polling: false,
})

const finishedListeners = new Set()
let timer = 0

const isActive = (job) => Boolean(job?.stage) && !FINISHED_STAGES.has(job.stage)
const isActiveStage = (stage) => Boolean(stage) && ACTIVE_STAGES.has(stage)

/** 订阅任务结束（完成或失败）事件，返回取消订阅函数。 */
export function onDictJobFinished(callback) {
  finishedListeners.add(callback)
  return () => finishedListeners.delete(callback)
}

const notifyFinished = (job) => {
  finishedListeners.forEach((callback) => {
    try {
      callback(job)
    } catch {
      /* 订阅者自己的错误不该影响轮询 */
    }
  })
}

async function tick() {
  let list = []
  try {
    list = await invoke('language_learning_dict_jobs')
  } catch {
    return
  }
  const map = {}
  for (const job of Array.isArray(list) ? list : []) {
    if (job?.id) map[job.id] = job
  }
  for (const [id, job] of Object.entries(map)) {
    // 只在「运行中 → 结束」这一跳通知一次，避免每轮都重复弹提示
    if (isActiveStage(dictJobs.map[id]?.stage) && FINISHED_STAGES.has(job.stage)) {
      notifyFinished(job)
    }
  }
  dictJobs.map = map
  if (!Object.values(map).some(isActive)) stopDictJobsPolling()
}

export function startDictJobsPolling() {
  if (timer) return
  timer = window.setInterval(tick, 900)
  dictJobs.polling = true
  tick()
}

export function stopDictJobsPolling() {
  if (!timer) return
  window.clearInterval(timer)
  timer = 0
  dictJobs.polling = false
}

export const hasActiveDictJob = () => Object.values(dictJobs.map).some(isActive)
