<template>
  <section class="tr-budget">
    <div class="tr-panel-head">
      <div>
        <div class="tr-eyebrow"><span></span> BUDGET LEDGER</div>
        <h2>花费清单</h2>
        <p>金额由路线、门票、餐饮档位和住宿晚数估算，可手动改成你实际能接受的价格。</p>
      </div>
      <div class="tr-budget-total">
        <small>合计</small>
        <strong>¥{{ money(total) }}</strong>
        <span>人均 ¥{{ money(perPerson) }}</span>
      </div>
    </div>

    <div class="tr-budget-bar">
      <span
        v-for="item in items"
        :key="item.category"
        :class="['tr-budget-seg', segClass(item.category)]"
        :style="{ width: `${percent(item.amount)}%` }"
        :title="`${item.category} ¥${money(item.amount)}`"
      ></span>
    </div>

    <div class="tr-budget-table">
      <div v-for="item in items" :key="item.category" class="tr-budget-row">
        <span class="tr-budget-dot" :class="segClass(item.category)"></span>
        <div class="tr-budget-label">
          <strong>{{ item.category }}</strong>
          <span>{{ item.label }}</span>
          <small v-if="item.note">{{ item.note }}</small>
        </div>
        <div class="tr-money-input is-inline">
          <span>¥</span>
          <input v-model.number="item.amount" type="number" min="0" step="10" @input="dirty = true" />
        </div>
      </div>
      <div class="tr-budget-row is-total">
        <span class="tr-budget-dot is-total"></span>
        <div class="tr-budget-label">
          <strong>总计</strong>
          <span>{{ people }} 人 · {{ nights }} 晚</span>
        </div>
        <div class="tr-budget-total-inline">¥{{ money(total) }}</div>
      </div>
    </div>

    <div class="tr-budget-foot">
      <div>
        <span v-if="plan.budget?.note" class="tr-budget-note">
          <TravelIcon name="info" :size="14" /> {{ plan.budget.note }}
        </span>
        <span v-else class="tr-budget-note">
          <TravelIcon name="info" :size="14" /> 预算是估算值；节假日酒店和机票可能明显上浮。
        </span>
      </div>
      <div class="tr-budget-actions">
        <button type="button" class="tr-btn is-ghost is-small" :disabled="!dirty" @click="reset">还原</button>
        <button type="button" class="tr-btn is-primary is-small" :disabled="!dirty" @click="save">保存修改</button>
      </div>
    </div>

    <div class="tr-budget-tips">
      <div><strong>怎么省</strong><span>住宿换到地铁沿线，门票优先选免费博物馆，正餐把一顿老字号换成本地小馆。</span></div>
      <div><strong>怎么花得值</strong><span>把预算留给最想看的那个景点或一顿预约制餐厅，其余行程保持松弛。</span></div>
    </div>
  </section>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { money } from './travelUtils.js'

const props = defineProps({
  plan: { type: Object, required: true },
})
const emit = defineEmits(['update'])

const items = ref((props.plan.budget?.items || []).map((item) => ({ ...item })))
const dirty = ref(false)

watch(
  () => props.plan.id,
  () => {
    items.value = (props.plan.budget?.items || []).map((item) => ({ ...item }))
    dirty.value = false
  },
)

const total = computed(() => items.value.reduce((sum, item) => sum + (Number(item.amount) || 0), 0))
const people = computed(
  () =>
    (props.plan.travelers?.adults || 0) +
    (props.plan.travelers?.children || 0) +
    (props.plan.travelers?.elders || 0),
)
const perPerson = computed(() => Math.round(total.value / Math.max(1, people.value)))
const nights = computed(() => Math.max(0, (props.plan.days?.length || 1) - 1))

function percent(value) {
  const amount = Number(value) || 0
  if (!total.value) return 0
  return Math.max(2, Math.round((amount / total.value) * 100))
}

function segClass(category) {
  return (
    {
      交通: 'seg-transport',
      门票: 'seg-ticket',
      餐饮: 'seg-food',
      住宿: 'seg-hotel',
      机动: 'seg-other',
    }[category] || 'seg-other'
  )
}

function reset() {
  items.value = (props.plan.budget?.items || []).map((item) => ({ ...item }))
  dirty.value = false
}

function save() {
  emit('update', items.value.map((item) => ({ ...item, amount: Number(item.amount) || 0 })))
  dirty.value = false
}
</script>
