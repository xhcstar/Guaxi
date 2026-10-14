// TTS 音色（Speaker ID）的共享定义。
//
// 之前这份 103 人的名单只存在于有声书页面里，设置页无从复用；抽出来之后
// 两处共用同一份来源，避免各写一份慢慢走样。

/**
 * Kokoro multi-lang 的说话人名单，下标即 speaker id。
 * 前缀含义：af_/bf_ 美式/英式女声，zf_/zm_ 中文女声/男声。
 */
export const KOKORO_SPEAKER_NAMES = [
  'af_maple', 'af_sol', 'bf_vale',
  'zf_001', 'zf_002', 'zf_003', 'zf_004', 'zf_005', 'zf_006', 'zf_007', 'zf_008', 'zf_017',
  'zf_018', 'zf_019', 'zf_021', 'zf_022', 'zf_023', 'zf_024', 'zf_026', 'zf_027', 'zf_028',
  'zf_032', 'zf_036', 'zf_038', 'zf_039', 'zf_040', 'zf_042', 'zf_043', 'zf_044', 'zf_046',
  'zf_047', 'zf_048', 'zf_049', 'zf_051', 'zf_059', 'zf_060', 'zf_067', 'zf_070', 'zf_071',
  'zf_072', 'zf_073', 'zf_074', 'zf_075', 'zf_076', 'zf_077', 'zf_078', 'zf_079', 'zf_083',
  'zf_084', 'zf_085', 'zf_086', 'zf_087', 'zf_088', 'zf_090', 'zf_092', 'zf_093', 'zf_094',
  'zf_099',
  'zm_009', 'zm_010', 'zm_011', 'zm_012', 'zm_013', 'zm_014', 'zm_015', 'zm_016', 'zm_020',
  'zm_025', 'zm_029', 'zm_030', 'zm_031', 'zm_033', 'zm_034', 'zm_035', 'zm_037', 'zm_041',
  'zm_045', 'zm_050', 'zm_052', 'zm_053', 'zm_054', 'zm_055', 'zm_056', 'zm_057', 'zm_058',
  'zm_061', 'zm_062', 'zm_063', 'zm_064', 'zm_065', 'zm_066', 'zm_068', 'zm_069', 'zm_080',
  'zm_081', 'zm_082', 'zm_089', 'zm_091', 'zm_095', 'zm_096', 'zm_097', 'zm_098', 'zm_100',
]

/** 非 Kokoro 模型兜底给出的编号范围：多数 VITS/Melo 只有 1 个音色，但允许填更大的值。 */
const GENERIC_SPEAKER_COUNT = 16

/** 与后端 `config.rs` 的判定保持一致：模型路径里带 kokoro 即为 Kokoro 系。 */
export function isKokoroModel(modelPath) {
  return String(modelPath || '').toLowerCase().includes('kokoro')
}

/**
 * 生成音色下拉选项。
 *
 * Kokoro 给分组+具名，中文/英文音色一目了然；其它模型只给编号，
 * 并由调用方开启 `tag` 允许手填编号（不同 VITS 模型的说话人数并不统一）。
 */
export function buildSpeakerOptions(modelPath) {
  if (!isKokoroModel(modelPath)) {
    return Array.from({ length: GENERIC_SPEAKER_COUNT }, (_, index) => ({
      label: `Speaker ${index}`,
      value: String(index),
    }))
  }

  const groups = {
    chineseFemale: [],
    chineseMale: [],
    americanFemale: [],
    britishFemale: [],
    other: [],
  }
  KOKORO_SPEAKER_NAMES.forEach((name, index) => {
    const option = { label: `${index} · ${name}`, value: String(index) }
    if (name.startsWith('zf_')) groups.chineseFemale.push(option)
    else if (name.startsWith('zm_')) groups.chineseMale.push(option)
    else if (name.startsWith('af_')) groups.americanFemale.push(option)
    else if (name.startsWith('bf_')) groups.britishFemale.push(option)
    else groups.other.push(option)
  })

  return [
    { type: 'group', label: '中文女声', key: 'chinese-female', children: groups.chineseFemale },
    { type: 'group', label: '中文男声', key: 'chinese-male', children: groups.chineseMale },
    { type: 'group', label: '美式女声', key: 'american-female', children: groups.americanFemale },
    { type: 'group', label: '英式女声', key: 'british-female', children: groups.britishFemale },
    { type: 'group', label: '其他', key: 'other', children: groups.other },
  ].filter((group) => group.children.length > 0)
}

/** 把 speaker id 还原成便于阅读的说明，用于设置页的提示行。 */
export function describeSpeaker(modelPath, speakerId) {
  const id = String(speakerId ?? '').trim() || '0'
  if (!isKokoroModel(modelPath)) {
    return `Speaker ${id}（当前模型非 Kokoro，多数只有 1 个音色）`
  }
  const index = Number(id)
  const name = Number.isInteger(index) ? KOKORO_SPEAKER_NAMES[index] : undefined
  return name ? `Speaker ${id} · ${name}` : `Speaker ${id}（该编号不在 Kokoro 名单内）`
}
