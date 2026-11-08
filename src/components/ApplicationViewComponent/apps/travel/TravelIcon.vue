<template>
  <Icon :icon="icons[name] || icons.compass" :width="size" :height="size" />
</template>

<script setup>
import { Icon } from '@iconify/vue'
import compass from '@iconify-icons/lucide/compass'
import map from '@iconify-icons/lucide/map'
import train from '@iconify-icons/lucide/train'
import car from '@iconify-icons/lucide/car-front'
import walk from '@iconify-icons/lucide/footprints'
import users from '@iconify-icons/lucide/users'
import calendar from '@iconify-icons/lucide/calendar-days'
import clock from '@iconify-icons/lucide/clock-3'
import wallet from '@iconify-icons/lucide/wallet'
import share from '@iconify-icons/lucide/share-2'
import print from '@iconify-icons/lucide/printer'
import settings from '@iconify-icons/lucide/settings'
import plus from '@iconify-icons/lucide/plus'
import arrowRight from '@iconify-icons/lucide/arrow-right'
import chevronLeft from '@iconify-icons/lucide/chevron-left'
import chevronRight from '@iconify-icons/lucide/chevron-right'
import check from '@iconify-icons/lucide/check'
import x from '@iconify-icons/lucide/x'
import refresh from '@iconify-icons/lucide/refresh-cw'
import lock from '@iconify-icons/lucide/lock'
import unlock from '@iconify-icons/lucide/unlock'
import sparkles from '@iconify-icons/lucide/sparkles'
import pin from '@iconify-icons/lucide/map-pin'
import sun from '@iconify-icons/lucide/sun'
import cloud from '@iconify-icons/lucide/cloud'
import rain from '@iconify-icons/lucide/cloud-rain'
import snow from '@iconify-icons/lucide/snowflake'
import storm from '@iconify-icons/lucide/cloud-lightning'
import fog from '@iconify-icons/lucide/cloud-fog'
import hotel from '@iconify-icons/lucide/bed-double'
import food from '@iconify-icons/lucide/utensils'
import route from '@iconify-icons/lucide/route'
import trash from '@iconify-icons/lucide/trash-2'
import download from '@iconify-icons/lucide/download'
import copy from '@iconify-icons/lucide/copy'
import qr from '@iconify-icons/lucide/qr-code'
import external from '@iconify-icons/lucide/external-link'
import info from '@iconify-icons/lucide/info'
import warning from '@iconify-icons/lucide/alert-triangle'
import backpack from '@iconify-icons/lucide/backpack'
import lightbulb from '@iconify-icons/lucide/lightbulb'
import flag from '@iconify-icons/lucide/flag'
import navigation from '@iconify-icons/lucide/navigation'
import book from '@iconify-icons/lucide/book-open'
import ticket from '@iconify-icons/lucide/ticket'
import camera from '@iconify-icons/lucide/camera'
import heart from '@iconify-icons/lucide/heart'
import globe from '@iconify-icons/lucide/globe'
import trainFront from '@iconify-icons/lucide/train-front'
import receipt from '@iconify-icons/lucide/receipt'

const icons = {
  compass,
  map,
  train,
  trainFront,
  car,
  walk,
  users,
  calendar,
  clock,
  wallet,
  share,
  print,
  settings,
  plus,
  arrowRight,
  chevronLeft,
  chevronRight,
  check,
  x,
  refresh,
  lock,
  unlock,
  sparkles,
  pin,
  sun,
  cloud,
  rain,
  snow,
  storm,
  fog,
  hotel,
  food,
  route,
  trash,
  download,
  copy,
  qr,
  external,
  info,
  warning,
  backpack,
  lightbulb,
  flag,
  navigation,
  book,
  ticket,
  camera,
  heart,
  globe,
  receipt,
}

defineProps({
  name: { type: String, default: 'compass' },
  size: { type: [Number, String], default: 18 },
})
</script>
