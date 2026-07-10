//! Story IR 1.0 的共享语义校验。
//!
//! JSON Schema 负责字段形状；这里进一步检查 ID 唯一性、跨字段引用与时长关系。
//! 校验器不调用模型、不访问文件，因此可以被编译入口和项目生命周期安全复用。

use serde_json::{json, Map, Value};
use std::collections::HashSet;

const MAX_SCENES: usize = 500;
const MAX_SHOTS: usize = 10_000;

fn required_id<'a>(object: &'a Map<String, Value>, path: &str) -> Result<&'a str, String> {
    let value = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{path}.id 不能为空"))?;
    if value.len() > 128 {
        return Err(format!("{path}.id 不能超过 128 字符"));
    }
    Ok(value)
}

fn collect_entity_ids(
    entities: &Map<String, Value>,
    field: &str,
    warnings: &mut Vec<String>,
) -> Result<HashSet<String>, String> {
    let mut ids = HashSet::new();
    let Some(items) = entities.get(field).and_then(Value::as_array) else {
        warnings.push(format!("entities.{field} 缺失，将按空列表处理"));
        return Ok(ids);
    };
    for (index, value) in items.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| format!("entities.{field}[{index}] 必须是对象"))?;
        let id = required_id(object, &format!("entities.{field}[{index}]"))?;
        if !ids.insert(id.to_string()) {
            return Err(format!("entities.{field} 存在重复 id：{id}"));
        }
    }
    Ok(ids)
}

/// 归一化并校验 Story IR。可恢复问题进入 warnings，破坏引用/执行确定性的问题返回错误。
pub fn validate_and_normalize_story_ir(
    mut story_ir: Value,
) -> Result<(Value, Vec<String>), String> {
    let root = story_ir
        .as_object_mut()
        .ok_or_else(|| "Story IR 根节点必须是 JSON 对象".to_string())?;
    root.insert("schemaVersion".to_string(), Value::String("1.0".to_string()));

    let metadata = root
        .entry("metadata".to_string())
        .or_insert_with(|| json!({}));
    let metadata = metadata
        .as_object_mut()
        .ok_or_else(|| "metadata 必须是对象".to_string())?;
    metadata.insert(
        "targetBlenderVersion".to_string(),
        Value::String("5.2 LTS".to_string()),
    );
    let fps = metadata.get("fps").and_then(Value::as_u64).unwrap_or(24);
    if !(12..=60).contains(&fps) {
        return Err("metadata.fps 必须在 12–60 之间".to_string());
    }
    metadata.insert("fps".to_string(), json!(fps));
    let target_duration = metadata
        .get("targetDurationSeconds")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0);

    let mut warnings = Vec::new();
    let empty_entities = Map::new();
    let entities = match root.get("entities") {
        Some(Value::Object(value)) => value,
        Some(_) => return Err("entities 必须是对象".to_string()),
        None => {
            warnings.push("entities 缺失，将按空实体集合处理".to_string());
            &empty_entities
        }
    };
    let character_ids = collect_entity_ids(entities, "characters", &mut warnings)?;
    let location_ids = collect_entity_ids(entities, "locations", &mut warnings)?;
    let _prop_ids = collect_entity_ids(entities, "props", &mut warnings)?;

    let scenes = root
        .get("scenes")
        .and_then(Value::as_array)
        .ok_or_else(|| "Story IR 缺少 scenes 数组".to_string())?;
    if scenes.is_empty() {
        return Err("Story IR 至少需要一个场景".to_string());
    }
    if scenes.len() > MAX_SCENES {
        return Err(format!("Story IR 场景数超过 {MAX_SCENES}，请按章节拆分"));
    }

    let mut scene_ids = HashSet::new();
    let mut shot_ids = HashSet::new();
    let mut shot_count = 0usize;
    let mut total_duration = 0.0f64;
    for (scene_index, scene) in scenes.iter().enumerate() {
        let path = format!("scenes[{scene_index}]");
        let scene_object = scene
            .as_object()
            .ok_or_else(|| format!("{path} 必须是对象"))?;
        let scene_id = required_id(scene_object, &path)?;
        if !scene_ids.insert(scene_id.to_string()) {
            return Err(format!("场景 id 重复：{scene_id}"));
        }
        let duration = scene_object
            .get("durationSeconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if duration <= 0.0 || duration > 3_600.0 {
            return Err(format!("{path}.durationSeconds 必须在 0–3600 秒之间"));
        }
        total_duration += duration;

        if let Some(location_id) = scene_object
            .get("locationId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !location_ids.contains(location_id) {
                warnings.push(format!("场景 {scene_id} 引用了未声明地点：{location_id}"));
            }
        }

        let shots = scene_object.get("shots").and_then(Value::as_array);
        let Some(shots) = shots else {
            warnings.push(format!("场景 {scene_id} 没有 shots 数组，将使用占位镜头"));
            continue;
        };
        if shots.is_empty() {
            warnings.push(format!("场景 {scene_id} 尚未生成镜头，将使用占位镜头"));
        }
        for (shot_index, shot) in shots.iter().enumerate() {
            shot_count += 1;
            if shot_count > MAX_SHOTS {
                return Err(format!("Story IR 镜头总数超过 {MAX_SHOTS}"));
            }
            let shot_path = format!("{path}.shots[{shot_index}]");
            let shot_object = shot
                .as_object()
                .ok_or_else(|| format!("{shot_path} 必须是对象"))?;
            let shot_id = required_id(shot_object, &shot_path)?;
            if !shot_ids.insert(shot_id.to_string()) {
                return Err(format!("镜头 id 重复：{shot_id}"));
            }
            let shot_duration = shot_object
                .get("durationSeconds")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if shot_duration <= 0.0 || shot_duration > duration + 0.001 {
                warnings.push(format!(
                    "镜头 {shot_id} 时长 {shot_duration} 秒无效或超过所属场景 {duration} 秒"
                ));
            }
            if let Some(references) = shot_object.get("characters").and_then(Value::as_array) {
                for reference in references.iter().filter_map(Value::as_str) {
                    if !character_ids.contains(reference) {
                        warnings.push(format!("镜头 {shot_id} 引用了未声明人物：{reference}"));
                    }
                }
            }
        }
    }

    if let Some(target) = target_duration {
        let difference_ratio = (total_duration - target).abs() / target.max(1.0);
        if difference_ratio > 0.25 {
            warnings.push(format!(
                "场景总时长 {total_duration:.1} 秒与目标 {target:.1} 秒相差超过 25%"
            ));
        }
    }

    if let Some(asset_plan) = root.get("assetPlan").and_then(Value::as_array) {
        let mut asset_ids = HashSet::new();
        for (index, asset) in asset_plan.iter().enumerate() {
            let object = asset
                .as_object()
                .ok_or_else(|| format!("assetPlan[{index}] 必须是对象"))?;
            let id = required_id(object, &format!("assetPlan[{index}]"))?;
            if !asset_ids.insert(id.to_string()) {
                return Err(format!("资产 id 重复：{id}"));
            }
        }
    }

    Ok((story_ir, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_scene_ids() {
        let value = json!({
            "metadata": {"fps": 24},
            "entities": {"characters": [], "locations": [], "props": []},
            "scenes": [
                {"id": "scene_1", "durationSeconds": 1, "shots": []},
                {"id": "scene_1", "durationSeconds": 1, "shots": []}
            ]
        });
        assert!(validate_and_normalize_story_ir(value)
            .unwrap_err()
            .contains("场景 id 重复"));
    }

    #[test]
    fn normalizes_blender_version_and_warns_unknown_character() {
        let value = json!({
            "metadata": {"fps": 24, "targetDurationSeconds": 2},
            "entities": {"characters": [], "locations": [], "props": []},
            "scenes": [{
                "id": "scene_1",
                "durationSeconds": 2,
                "shots": [{
                    "id": "shot_1",
                    "durationSeconds": 2,
                    "characters": ["missing"]
                }]
            }]
        });
        let (normalized, warnings) = validate_and_normalize_story_ir(value).unwrap();
        assert_eq!(
            normalized.pointer("/metadata/targetBlenderVersion"),
            Some(&Value::String("5.2 LTS".to_string()))
        );
        assert!(warnings.iter().any(|warning| warning.contains("未声明人物")));
    }
}
