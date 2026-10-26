<template>
  <img
    v-if="url"
    class="rrt-img"
    :src="url"
    :alt="node?.name || ''"
    loading="lazy"
    draggable="false"
    @error="onError"
  />
  <span v-else class="rrt-fallback" :data-failed="failed">
    <Icon :icon="failed ? brokenIcon : imageIcon" width="18" height="18" />
  </span>
</template>

<script setup>
import { onBeforeUnmount, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import imageIcon from '@iconify-icons/lucide/image'
import brokenIcon from '@iconify-icons/lucide/image-off'
import { loadThumbnail, peekThumbnail } from './resourceThumbs'
import { PREVIEWABLE_IMAGE_EXTS, extOf } from './resourceRepoModel'

const props = defineProps({
  node: { type: Object, default: null },
  root: { type: String, default: '' },
  edge: { type: Number, default: 160 },
})

const url = ref('')
const failed = ref(false)
let token = 0

const supportsThumb = (name) => {
  const ext = extOf(name)
  return ext !== 'svg' && PREVIEWABLE_IMAGE_EXTS.has(ext)
}

const load = () => {
  const node = props.node
  token += 1
  const current = token
  url.value = ''
  failed.value = false
  if (!node || node.isDir || !supportsThumb(node.name)) {
    failed.value = true
    return
  }

  const hit = peekThumbnail(node.path, node.mtime, props.edge)
  if (hit) {
    url.value = hit.url
    return
  }

  loadThumbnail(node.path, node.mtime, props.edge)
    .then((value) => {
      if (current !== token) return
      url.value = value.url
      failed.value = false
    })
    .catch(() => {
      if (current !== token) return
      failed.value = true
    })
}

const onError = () => {
  failed.value = true
  url.value = ''
}

watch(() => [props.node?.path, props.node?.mtime], load, { immediate: true })

onBeforeUnmount(() => {
  token += 1
})
</script>

<style scoped>
.rrt-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  border-radius: 7px;
}

.rrt-fallback {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  border-radius: 7px;
  color: var(--guaxi-text-secondary, #57606a);
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 6%, transparent);
}

.rrt-fallback[data-failed='true'] {
  opacity: 0.6;
}
</style>
