import argparse
import concurrent.futures
import json
import os
import sys
from pathlib import Path
from urllib.request import Request, urlopen


def _strip_s3_prefix(s: str) -> str:
    prefix = "s3://"
    return s[len(prefix) :] if s.startswith(prefix) else s


def _join_url(base: str, path: str) -> str:
    return base.rstrip("/") + "/" + path.lstrip("/")


def _http_get_bytes(url: str) -> bytes:
    req = Request(url, headers={"User-Agent": "guaxi-surya-downloader"})
    with urlopen(req, timeout=60) as resp:
        return resp.read()


def _download_file(url: str, dest_path: Path) -> None:
    dest_path.parent.mkdir(parents=True, exist_ok=True)

    req = Request(url, headers={"User-Agent": "guaxi-surya-downloader"})
    with urlopen(req, timeout=60) as resp:
        tmp_path = dest_path.with_suffix(dest_path.suffix + ".part")
        with open(tmp_path, "wb") as f:
            while True:
                chunk = resp.read(1024 * 1024)
                if not chunk:
                    break
                f.write(chunk)
        os.replace(tmp_path, dest_path)


def _download_directory(base_url: str, remote_dir: str, local_dir: Path, workers: int) -> None:
    local_dir.mkdir(parents=True, exist_ok=True)
    manifest_url = _join_url(_join_url(base_url, remote_dir), "manifest.json")
    manifest = json.loads(_http_get_bytes(manifest_url).decode("utf-8"))
    files = manifest.get("files") or []

    missing: list[tuple[str, Path]] = []
    for rel in files:
        dest = local_dir / rel
        if dest.exists():
            continue
        missing.append((rel, dest))

    if not missing:
        return

    print(f"Downloading {remote_dir} -> {local_dir} ({len(missing)}/{len(files)} files)")

    def _task(rel_and_dest: tuple[str, Path]) -> None:
        rel, dest = rel_and_dest
        url = _join_url(_join_url(base_url, remote_dir), rel)
        _download_file(url, dest)

    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as ex:
        futs = [ex.submit(_task, item) for item in missing]
        for fut in concurrent.futures.as_completed(futs):
            fut.result()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Download Surya models into a local folder, without importing surya."
    )
    parser.add_argument(
        "--dest",
        default=".",
        help="Destination base directory (default: current directory).",
    )
    parser.add_argument(
        "--dir",
        default="surya_models",
        help="Subdirectory under --dest to store models (default: surya_models).",
    )
    parser.add_argument(
        "--base-url",
        default="https://models.datalab.to",
        help="Model base URL (default: https://models.datalab.to).",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=10,
        help="Parallel download workers (default: 10).",
    )
    args = parser.parse_args()

    dest_dir = (Path(args.dest).resolve() / args.dir).resolve()
    dest_dir.mkdir(parents=True, exist_ok=True)

    checkpoints = [
        ("DETECTOR_MODEL_CHECKPOINT", "s3://text_detection/2025_05_07"),
        ("FOUNDATION_MODEL_CHECKPOINT", "s3://text_recognition/2025_09_23"),
        ("RECOGNITION_MODEL_CHECKPOINT", "s3://text_recognition/2025_09_23"),
        ("LAYOUT_MODEL_CHECKPOINT", "s3://layout/2025_09_23"),
        ("TABLE_REC_MODEL_CHECKPOINT", "s3://table_recognition/2025_02_18"),
        ("OCR_ERROR_MODEL_CHECKPOINT", "s3://ocr_error_detection/2025_02_18"),
    ]

    print(f"Python     = {sys.executable}")
    print(f"Base URL   = {args.base_url}")
    print(f"Dest Dir   = {dest_dir}")
    print(f"Workers    = {args.workers}")
    print("")

    for name, ckpt in checkpoints:
        remote = _strip_s3_prefix(ckpt)
        local_dir = dest_dir / remote
        print(f"[{name}] {ckpt}")
        _download_directory(args.base_url, remote, local_dir, args.workers)

    print("")
    print(f"Done. Models are under: {dest_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())