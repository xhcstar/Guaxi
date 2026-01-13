import os

os.environ["GRPC_VERBOSITY"] = "ERROR"
os.environ["GLOG_minloglevel"] = "2"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = (
    "1"  # Transformers uses .isin for a simple op, which is not supported on MPS
)

import json
import time
import traceback

import click

from marker.config.parser import ConfigParser
from marker.config.printer import CustomClickPrinter
from marker.logger import configure_logging, get_logger
from marker.output import save_output
from marker.settings import settings

configure_logging()
logger = get_logger()


def _emit_progress(enabled: bool, current: int, total: int, message: str):
    if not enabled:
        return
    print(
        json.dumps(
            {
                "type": "conversion_progress",
                "current": int(current),
                "total": int(total),
                "message": str(message),
            },
            ensure_ascii=False,
        ),
        flush=True,
    )


def _load_models(progress_json: bool):
    from surya.foundation import FoundationPredictor
    from surya.detection import DetectionPredictor
    from surya.layout import LayoutPredictor
    from surya.ocr_error import OCRErrorPredictor
    from surya.recognition import RecognitionPredictor
    from surya.table_rec import TableRecPredictor
    from surya.settings import settings as surya_settings

    device = settings.TORCH_DEVICE_MODEL
    dtype = settings.MODEL_DTYPE

    _emit_progress(progress_json, 5, 100, "加载模型: layout")
    layout_model = LayoutPredictor(
        FoundationPredictor(
            checkpoint=surya_settings.LAYOUT_MODEL_CHECKPOINT,
            attention_implementation=None,
            device=device,
            dtype=dtype,
        )
    )

    _emit_progress(progress_json, 15, 100, "加载模型: recognition")
    recognition_model = RecognitionPredictor(
        FoundationPredictor(
            checkpoint=surya_settings.RECOGNITION_MODEL_CHECKPOINT,
            attention_implementation=None,
            device=device,
            dtype=dtype,
        )
    )

    _emit_progress(progress_json, 25, 100, "加载模型: table_rec")
    table_rec_model = TableRecPredictor(device=device, dtype=dtype)

    _emit_progress(progress_json, 30, 100, "加载模型: detection")
    detection_model = DetectionPredictor(device=device, dtype=dtype)

    _emit_progress(progress_json, 35, 100, "加载模型: ocr_error")
    ocr_error_model = OCRErrorPredictor(device=device, dtype=dtype)

    _emit_progress(progress_json, 40, 100, "模型加载完成")

    return {
        "layout_model": layout_model,
        "recognition_model": recognition_model,
        "table_rec_model": table_rec_model,
        "detection_model": detection_model,
        "ocr_error_model": ocr_error_model,
    }


@click.command(cls=CustomClickPrinter, help="Convert a single PDF to markdown.")
@click.argument("fpath", type=str)
@click.option("--paginate_output", is_flag=True, default=False)
@click.option("--progress_json", is_flag=True, default=False)
@ConfigParser.common_options
def convert_single_cli(fpath: str, paginate_output: bool, progress_json: bool, **kwargs):
    start = time.time()
    _emit_progress(progress_json, 0, 100, "开始转换")

    try:
        models = _load_models(progress_json)
        _emit_progress(progress_json, 45, 100, "初始化转换器")

        kwargs["paginate_output"] = paginate_output
        config_parser = ConfigParser(kwargs)

        converter_cls = config_parser.get_converter_cls()
        converter = converter_cls(
            config=config_parser.generate_config_dict(),
            artifact_dict=models,
            processor_list=config_parser.get_processors(),
            renderer=config_parser.get_renderer(),
            llm_service=config_parser.get_llm_service(),
        )

        _emit_progress(progress_json, 55, 100, "正在转换")
        rendered = converter(fpath)

        _emit_progress(progress_json, 90, 100, "正在写入结果")
        out_folder = config_parser.get_output_folder(fpath)
        save_output(rendered, out_folder, config_parser.get_base_filename(fpath))

        _emit_progress(progress_json, 100, 100, "转换完成")
        logger.info(f"Saved markdown to {out_folder}")
        logger.info(f"Total time: {time.time() - start}")
    except Exception as e:
        _emit_progress(progress_json, 100, 100, f"转换失败: {e}")
        traceback.print_exc()
        raise
