"""Guaxi Blender Bridge 5.2 LTS 插件。

HTTP 线程只负责鉴权和排队；所有 bpy 操作都由 bpy.app.timers 在主线程执行。
这条边界是插件稳定性的关键，也避免远端请求携带任意 Python 进入 Blender。
"""

import json
import queue
import secrets
import threading
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import bpy
from bpy.props import IntProperty, StringProperty

# 注意：此文件不能启用 `from __future__ import annotations`。Blender 通过类的
# __annotations__ 读取 IntProperty/StringProperty 返回的延迟属性对象；把注解字符串化后
# Preferences 属性将无法注册。Blender 5.2 自带的 Python 已支持下方使用的联合类型语法。

from .scene_builder import (
    build_story_asset_preview,
    build_story_preview,
    render_build_plan,
)


bl_info = {
    "name": "Guaxi Blender Bridge",
    "author": "Guaxi",
    "version": (0, 1, 0),
    "blender": (5, 2, 0),
    "location": "3D Viewport > Sidebar > Guaxi",
    "description": "Controlled Story IR bridge for the Guaxi AI film studio",
    "category": "Pipeline",
}


PROTOCOL_VERSION = "1.0"
MAX_REQUEST_BYTES = 1024 * 1024
MAX_OPERATIONS = 100
ALLOWED_OPERATIONS = {
    "ping",
    "set_frame",
    "select_object",
    "set_object_transform",
    "set_world_color",
    "build_story_ir",
    "build_story_assets",
    "render_build_plan",
    "save_project",
}

_COMMAND_QUEUE: queue.Queue[tuple[dict[str, Any], queue.Queue[dict[str, Any]]]] = queue.Queue()
_SERVER: ThreadingHTTPServer | None = None
_SERVER_THREAD: threading.Thread | None = None
_SERVER_LOCK = threading.Lock()


def _log(level: str, message: str, **fields: Any) -> None:
    """统一控制台日志格式，方便 Guaxi 和用户在 Blender 控制台检索。"""
    suffix = f" {json.dumps(fields, ensure_ascii=False, default=str)}" if fields else ""
    print(f"[GuaxiBridge][{level.upper()}] {message}{suffix}", flush=True)


def _addon_preferences() -> "GuaxiBridgePreferences":
    return bpy.context.preferences.addons[__package__].preferences


def _workspace_path(raw: str) -> Path:
    value = str(raw or "").strip()
    if not value:
        raise ValueError("请先在插件设置中配置 Guaxi 项目工作区")
    path = Path(value).expanduser().resolve()
    path.mkdir(parents=True, exist_ok=True)
    return path


def _path_inside_workspace(workspace: Path, raw: Any) -> Path:
    """所有可写路径都必须位于用户配置的项目工作区，阻断路径穿越。"""
    candidate = Path(str(raw or "")).expanduser()
    if not candidate.is_absolute():
        candidate = workspace / candidate
    resolved = candidate.resolve()
    try:
        resolved.relative_to(workspace)
    except ValueError as exc:
        raise ValueError(f"路径超出 Guaxi 工作区：{resolved}") from exc
    return resolved


def _vector(value: Any, length: int, minimum: float, maximum: float) -> list[float] | None:
    if value is None:
        return None
    if not isinstance(value, list) or len(value) != length:
        raise ValueError(f"向量必须包含 {length} 个数字")
    result = []
    for item in value:
        number = float(item)
        result.append(max(minimum, min(maximum, number)))
    return result


def _execute_operation(operation: dict[str, Any], workspace: Path) -> dict[str, Any]:
    operation_type = str(operation.get("type", "")).strip()
    if operation_type not in ALLOWED_OPERATIONS:
        raise ValueError(f"不支持的操作：{operation_type}")

    if operation_type == "ping":
        return {"type": operation_type, "ok": True, "blenderVersion": bpy.app.version_string}

    if operation_type == "set_frame":
        frame = int(operation.get("frame", 1))
        frame = max(bpy.context.scene.frame_start, min(bpy.context.scene.frame_end, frame))
        bpy.context.scene.frame_set(frame)
        return {"type": operation_type, "frame": frame}

    if operation_type == "select_object":
        object_id = str(operation.get("objectId", "")).strip()
        target = bpy.data.objects.get(object_id)
        if target is None:
            raise ValueError(f"找不到对象：{object_id}")
        bpy.ops.object.select_all(action="DESELECT")
        target.select_set(True)
        bpy.context.view_layer.objects.active = target
        return {"type": operation_type, "objectId": target.name}

    if operation_type == "set_object_transform":
        object_id = str(operation.get("objectId", "")).strip()
        target = bpy.data.objects.get(object_id)
        if target is None:
            raise ValueError(f"找不到对象：{object_id}")
        location = _vector(operation.get("location"), 3, -100_000.0, 100_000.0)
        rotation = _vector(operation.get("rotationEuler"), 3, -100.0, 100.0)
        scale = _vector(operation.get("scale"), 3, 0.0001, 10_000.0)
        if location is not None:
            target.location = location
        if rotation is not None:
            target.rotation_euler = rotation
        if scale is not None:
            target.scale = scale
        bpy.context.view_layer.update()
        return {"type": operation_type, "objectId": target.name}

    if operation_type == "set_world_color":
        color = _vector(operation.get("color"), 3, 0.0, 1.0)
        if color is None:
            raise ValueError("set_world_color 缺少 color")
        if bpy.context.scene.world is None:
            bpy.context.scene.world = bpy.data.worlds.new("GUAXI_WORLD")
        bpy.context.scene.world.color = color
        return {"type": operation_type, "color": color}

    if operation_type == "build_story_ir":
        story_ir = operation.get("storyIr")
        if not isinstance(story_ir, dict):
            raise ValueError("build_story_ir.storyIr 必须是 JSON 对象")
        project_dir = _path_inside_workspace(workspace, operation.get("projectDir"))
        outputs = build_story_preview(
            story_ir,
            project_dir,
            render_animation=bool(operation.get("renderAnimation", False)),
            progress=lambda value, stage, message: _log(
                "info", message, progress=value, stage=stage
            ),
        )
        return {"type": operation_type, **outputs}

    if operation_type == "build_story_assets":
        # 默认确定性资产化路径：解析真实资产 + 冻结 Build Plan + 重放（§7.2/§7.3）。
        story_ir = operation.get("storyIr")
        if not isinstance(story_ir, dict):
            raise ValueError("build_story_assets.storyIr 必须是 JSON 对象")
        project_dir = _path_inside_workspace(workspace, operation.get("projectDir"))
        outputs = build_story_asset_preview(
            story_ir,
            project_dir,
            render_animation=bool(operation.get("renderAnimation", False)),
            progress=lambda value, stage, message: _log(
                "info", message, progress=value, stage=stage
            ),
        )
        return {"type": operation_type, **outputs}

    if operation_type == "render_build_plan":
        # 重放一个已冻结的 Build Plan，确定性重渲染，不再解析或调用 LLM。
        build_plan = operation.get("buildPlan")
        if not isinstance(build_plan, dict):
            raise ValueError("render_build_plan.buildPlan 必须是 JSON 对象")
        project_dir = _path_inside_workspace(workspace, operation.get("projectDir"))
        outputs = render_build_plan(
            build_plan,
            project_dir,
            render_animation=bool(operation.get("renderAnimation", False)),
            progress=lambda value, stage, message: _log(
                "info", message, progress=value, stage=stage
            ),
        )
        return {"type": operation_type, **outputs}

    if operation_type == "save_project":
        filepath = _path_inside_workspace(workspace, operation.get("filepath"))
        if filepath.suffix.lower() != ".blend":
            raise ValueError("Blender 工程输出必须使用 .blend 扩展名")
        filepath.parent.mkdir(parents=True, exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=str(filepath), check_existing=False)
        return {"type": operation_type, "filepath": str(filepath)}

    raise ValueError(f"操作尚未实现：{operation_type}")


def _process_request(payload: dict[str, Any], workspace: Path) -> dict[str, Any]:
    request_id = str(payload.get("requestId", "")).strip()
    if payload.get("protocolVersion") != PROTOCOL_VERSION:
        raise ValueError(f"协议版本必须为 {PROTOCOL_VERSION}")
    if not request_id or len(request_id) > 128:
        raise ValueError("requestId 不能为空且不能超过 128 字符")
    operations = payload.get("operations")
    if not isinstance(operations, list) or not operations:
        raise ValueError("operations 必须是非空数组")
    if len(operations) > MAX_OPERATIONS:
        raise ValueError(f"单次请求最多 {MAX_OPERATIONS} 个操作")

    results = []
    for index, operation in enumerate(operations):
        if not isinstance(operation, dict):
            raise ValueError(f"第 {index + 1} 个操作不是 JSON 对象")
        results.append(_execute_operation(operation, workspace))
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "ok": True,
        "results": results,
    }


def _timer_process_commands() -> float | None:
    """Blender 主线程定时器：一次最多处理 10 个请求，避免长队列冻结 UI。"""
    with _SERVER_LOCK:
        running = _SERVER is not None
    if not running:
        return None

    processed = 0
    while processed < 10:
        try:
            payload, response_queue = _COMMAND_QUEUE.get_nowait()
        except queue.Empty:
            break
        try:
            workspace = _workspace_path(_addon_preferences().workspace_root)
            response = _process_request(payload, workspace)
            _log("info", "command completed", requestId=payload.get("requestId"))
        except Exception as exc:  # 错误被结构化返回，不让单个请求终止定时器。
            _log("error", "command failed", error=str(exc), requestId=payload.get("requestId"))
            response = {
                "protocolVersion": PROTOCOL_VERSION,
                "requestId": str(payload.get("requestId", "")),
                "ok": False,
                "error": str(exc),
            }
        response_queue.put(response)
        processed += 1
    return 0.1


class _BridgeRequestHandler(BaseHTTPRequestHandler):
    server_version = "GuaxiBlenderBridge/0.1"

    def _send_json(self, status: HTTPStatus, payload: dict[str, Any]) -> None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status.value)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def _authorized(self) -> bool:
        expected = getattr(self.server, "guaxi_token", "")
        provided = self.headers.get("Authorization", "")
        return bool(expected) and secrets.compare_digest(provided, f"Bearer {expected}")

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path == "/health":
            self._send_json(
                HTTPStatus.OK,
                {
                    "ok": True,
                    "service": "guaxi-blender-bridge",
                    "protocolVersion": PROTOCOL_VERSION,
                    # 版本号在主线程启动服务时固化，HTTP 线程不直接读取 bpy RNA/API。
                    "blenderVersion": getattr(self.server, "guaxi_blender_version", "5.2"),
                },
            )
            return
        self._send_json(HTTPStatus.NOT_FOUND, {"ok": False, "error": "not found"})

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path != "/v1/commands":
            self._send_json(HTTPStatus.NOT_FOUND, {"ok": False, "error": "not found"})
            return
        if not self._authorized():
            self._send_json(HTTPStatus.UNAUTHORIZED, {"ok": False, "error": "unauthorized"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if length <= 0 or length > MAX_REQUEST_BYTES:
            self._send_json(HTTPStatus.BAD_REQUEST, {"ok": False, "error": "invalid body size"})
            return
        try:
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
            if not isinstance(payload, dict):
                raise ValueError("JSON 根节点必须是对象")
        except Exception as exc:
            self._send_json(HTTPStatus.BAD_REQUEST, {"ok": False, "error": str(exc)})
            return

        response_queue: queue.Queue[dict[str, Any]] = queue.Queue(maxsize=1)
        _COMMAND_QUEUE.put((payload, response_queue))
        try:
            response = response_queue.get(timeout=120.0)
        except queue.Empty:
            self._send_json(HTTPStatus.GATEWAY_TIMEOUT, {"ok": False, "error": "Blender 主线程执行超时"})
            return
        self._send_json(HTTPStatus.OK if response.get("ok") else HTTPStatus.BAD_REQUEST, response)

    def log_message(self, format: str, *args: Any) -> None:
        _log("debug", format % args)


def _start_server(port: int, token: str) -> None:
    global _SERVER, _SERVER_THREAD
    with _SERVER_LOCK:
        if _SERVER is not None:
            raise RuntimeError("Guaxi Bridge 已经启动")
        server = ThreadingHTTPServer(("127.0.0.1", port), _BridgeRequestHandler)
        server.daemon_threads = True
        server.guaxi_token = token  # type: ignore[attr-defined]
        server.guaxi_blender_version = bpy.app.version_string  # type: ignore[attr-defined]
        thread = threading.Thread(target=server.serve_forever, name="GuaxiBridgeHTTP", daemon=True)
        _SERVER = server
        _SERVER_THREAD = thread
        thread.start()
    if not bpy.app.timers.is_registered(_timer_process_commands):
        bpy.app.timers.register(_timer_process_commands, first_interval=0.1, persistent=True)
    _log("info", "bridge started", host="127.0.0.1", port=port)


def _stop_server() -> None:
    global _SERVER, _SERVER_THREAD
    with _SERVER_LOCK:
        server = _SERVER
        thread = _SERVER_THREAD
        _SERVER = None
        _SERVER_THREAD = None
    if server is not None:
        server.shutdown()
        server.server_close()
    if thread is not None and thread.is_alive():
        thread.join(timeout=2.0)
    _log("info", "bridge stopped")


class GuaxiBridgePreferences(bpy.types.AddonPreferences):
    bl_idname = __package__

    port: IntProperty(
        name="端口",
        description="只监听 127.0.0.1",
        default=19152,
        min=1024,
        max=65535,
    )
    access_token: StringProperty(
        name="访问令牌",
        description="Guaxi 调用 /v1/commands 时使用的 Bearer Token",
        subtype="PASSWORD",
        default="",
    )
    workspace_root: StringProperty(
        name="Guaxi 项目工作区",
        description="插件只允许在该目录内读取或写入项目文件",
        subtype="DIR_PATH",
        default="",
    )

    def draw(self, _context: bpy.types.Context) -> None:
        layout = self.layout
        layout.prop(self, "workspace_root")
        layout.prop(self, "port")
        layout.prop(self, "access_token")
        layout.label(text="服务仅监听 127.0.0.1；令牌不会写入 Story IR。", icon="LOCKED")


class GUAXI_OT_generate_token(bpy.types.Operator):
    bl_idname = "guaxi.generate_bridge_token"
    bl_label = "生成访问令牌"
    bl_description = "生成新的本地桥接 Bearer Token"

    def execute(self, _context: bpy.types.Context) -> set[str]:
        prefs = _addon_preferences()
        prefs.access_token = secrets.token_urlsafe(32)
        _log("info", "new access token generated")
        self.report({"INFO"}, "已生成新的 Guaxi Bridge 访问令牌")
        return {"FINISHED"}


class GUAXI_OT_start_bridge(bpy.types.Operator):
    bl_idname = "guaxi.start_bridge"
    bl_label = "启动 Guaxi Bridge"

    def execute(self, _context: bpy.types.Context) -> set[str]:
        prefs = _addon_preferences()
        try:
            _workspace_path(prefs.workspace_root)
            if not prefs.access_token:
                prefs.access_token = secrets.token_urlsafe(32)
            _start_server(prefs.port, prefs.access_token)
        except Exception as exc:
            _log("error", "failed to start bridge", error=str(exc))
            self.report({"ERROR"}, str(exc))
            return {"CANCELLED"}
        self.report({"INFO"}, f"Guaxi Bridge 已启动：127.0.0.1:{prefs.port}")
        return {"FINISHED"}


class GUAXI_OT_stop_bridge(bpy.types.Operator):
    bl_idname = "guaxi.stop_bridge"
    bl_label = "停止 Guaxi Bridge"

    def execute(self, _context: bpy.types.Context) -> set[str]:
        _stop_server()
        self.report({"INFO"}, "Guaxi Bridge 已停止")
        return {"FINISHED"}


class GUAXI_PT_bridge_panel(bpy.types.Panel):
    bl_label = "Guaxi AI 影像工坊"
    bl_idname = "GUAXI_PT_bridge_panel"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "Guaxi"

    def draw(self, _context: bpy.types.Context) -> None:
        layout = self.layout
        prefs = _addon_preferences()
        with _SERVER_LOCK:
            running = _SERVER is not None
        status = layout.box()
        status.label(
            text="Bridge 运行中" if running else "Bridge 未启动",
            icon="CHECKMARK" if running else "PAUSE",
        )
        status.label(text=f"协议 {PROTOCOL_VERSION} · Blender {bpy.app.version_string}")
        if running:
            status.operator(GUAXI_OT_stop_bridge.bl_idname, icon="CANCEL")
        else:
            status.operator(GUAXI_OT_start_bridge.bl_idname, icon="PLAY")
        layout.operator(GUAXI_OT_generate_token.bl_idname, icon="FILE_REFRESH")
        layout.label(text=f"127.0.0.1:{prefs.port}", icon="URL")
        layout.label(text="项目路径与令牌在插件设置中配置", icon="PREFERENCES")


CLASSES = (
    GuaxiBridgePreferences,
    GUAXI_OT_generate_token,
    GUAXI_OT_start_bridge,
    GUAXI_OT_stop_bridge,
    GUAXI_PT_bridge_panel,
)


def register() -> None:
    for cls in CLASSES:
        bpy.utils.register_class(cls)
    _log("info", "addon registered", blenderVersion=bpy.app.version_string)


def unregister() -> None:
    _stop_server()
    if bpy.app.timers.is_registered(_timer_process_commands):
        bpy.app.timers.unregister(_timer_process_commands)
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
    _log("info", "addon unregistered")
