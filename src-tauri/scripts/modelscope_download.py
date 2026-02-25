import argparse
import sys
from pathlib import Path
import shutil

SCRIPT_DIR = Path(__file__).resolve().parent


def ensure_embedded_libs() -> list[str]:
    lib_root = SCRIPT_DIR.parent / "utils" / "python_libs"
    added: list[str] = []
    if lib_root.is_dir():
        sys.path.insert(0, str(lib_root))
        added.append(str(lib_root))
        site_packages = lib_root / "site-packages"
        if site_packages.is_dir():
            sys.path.insert(0, str(site_packages))
            added.append(str(site_packages))
    if added:
        print(f"extra_sys_path={added}")
    return added

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--local-dir", required=True)
    parser.add_argument("--filename", default="")
    parser.add_argument("--revision", default="")
    args = parser.parse_args()

    model_id = args.model_id.strip()
    local_dir = Path(args.local_dir).resolve()
    filename = args.filename.strip()
    revision = args.revision.strip()

    if not model_id:
        print("model_id is empty", file=sys.stderr)
        return 2

    local_dir.mkdir(parents=True, exist_ok=True)
    download_root = local_dir / "_modelscope_download"
    download_root.mkdir(parents=True, exist_ok=True)

    print(f"python={sys.executable}")
    print(f"model_id={model_id}")
    print(f"local_dir={local_dir}")
    print(f"download_root={download_root}")
    if revision:
        print(f"revision={revision}")

    ensure_embedded_libs()

    try:
        from modelscope import snapshot_download
    except Exception as e:
        print(f"import modelscope failed: {e}", file=sys.stderr)
        print(
            f"expected bundled libs at: {(SCRIPT_DIR.parent / 'utils' / 'python_libs').resolve()}",
            file=sys.stderr,
        )
        return 3

    kwargs = {"local_dir": str(download_root)}
    if revision:
        kwargs["revision"] = revision
    if filename:
        kwargs["allow_patterns"] = [f"**/{filename}"]

    try:
        download_path = snapshot_download(model_id, **kwargs)
    except TypeError as e:
        err_text = str(e)
        if "allow_patterns" in err_text:
            kwargs.pop("allow_patterns", None)
            download_path = snapshot_download(model_id, **kwargs)
        else:
            raise
    if not download_path:
        print("snapshot_download returned empty path", file=sys.stderr)
        return 4

    download_path = Path(download_path)

    if filename:
        model_file = None
        for p in download_path.rglob(filename):
            if p.is_file():
                model_file = p
                break

        if model_file is None:
            print(f"model file not found: {filename}", file=sys.stderr)
            return 5

        target = local_dir / filename
        tmp = target.with_suffix(target.suffix + ".download")
        if tmp.exists():
            tmp.unlink()
        shutil.copy2(model_file, tmp)
        tmp.replace(target)
        print(f"model_file={target}")
    else:
        print(f"download_dir={download_path}")

    return 0

if __name__ == "__main__":
    raise SystemExit(main())