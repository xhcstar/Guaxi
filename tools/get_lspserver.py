import json
import fnmatch
import shutil
import zipfile
from pathlib import Path
from urllib.request import Request, urlopen

BASE_DIR = Path(__file__).resolve().parents[1]
UTILS_DIR = BASE_DIR / "utils"
DOWNLOADS_DIR = UTILS_DIR / "lsp_downloads"
LSP_DIR = BASE_DIR / "lsp"


def ensure_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def get_latest_asset_url(repo: str, asset_pattern: str) -> tuple[str, str]:
    url = f"https://api.github.com/repos/{repo}/releases/latest"
    req = Request(url, headers={"User-Agent": "guaxi-lsp-downloader"})
    with urlopen(req) as resp:
        data = resp.read().decode("utf-8")
    release = json.loads(data)
    assets = release.get("assets", [])
    for asset in assets:
        name = asset.get("name", "")
        if fnmatch.fnmatch(name, asset_pattern):
            return asset.get("browser_download_url"), name
    raise RuntimeError(f"no asset matching '{asset_pattern}' in {repo} latest release")


def download_and_install(repo: str, asset_pattern: str, inner_exe_pattern: str, out_exe_name: str) -> None:
    ensure_dir(DOWNLOADS_DIR)
    ensure_dir(LSP_DIR)

    print(f"query latest release for {repo} ...")
    asset_url, asset_name = get_latest_asset_url(repo, asset_pattern)
    zip_path = DOWNLOADS_DIR / asset_name

    print(f"downloading {asset_name} -> {zip_path}")
    req = Request(asset_url, headers={"User-Agent": "guaxi-lsp-downloader"})
    with urlopen(req) as resp, open(zip_path, "wb") as f:
        shutil.copyfileobj(resp, f)

    extract_dir = DOWNLOADS_DIR / (asset_name + "_extract")
    ensure_dir(extract_dir)

    print(f"extracting to {extract_dir}")
    with zipfile.ZipFile(zip_path, "r") as zf:
        zf.extractall(extract_dir)

    print(f"searching for {inner_exe_pattern}")
    exe_path: Path | None = None
    for p in extract_dir.rglob(inner_exe_pattern):
        exe_path = p
        break
    if exe_path is None:
        raise RuntimeError(f"cannot find {inner_exe_pattern} in extracted {asset_name}")

    dest = LSP_DIR / out_exe_name
    print(f"copying {exe_path} -> {dest}")
    ensure_dir(dest.parent)
    shutil.copy2(exe_path, dest)

    print(f"installed {out_exe_name}")


def main() -> None:
    ensure_dir(UTILS_DIR)
    ensure_dir(DOWNLOADS_DIR)
    ensure_dir(LSP_DIR)

    # download_and_install(
    #     repo="llvm/llvm-project",
    #     asset_pattern="clang+llvm-*-win64*.zip",
    #     inner_exe_pattern="clangd.exe",
    #     out_exe_name="clangd.exe",
    # )

    # download_and_install(
    #     repo="rust-lang/rust-analyzer",
    #     asset_pattern="rust-analyzer-*x86_64-pc-windows-msvc.zip",
    #     inner_exe_pattern="rust-analyzer.exe",
    #     out_exe_name="rust-analyzer.exe",
    # )

    download_and_install(
        repo="LuaLS/lua-language-server",
        asset_pattern="*win32-x64.zip",
        inner_exe_pattern="lua-language-server.exe",
        out_exe_name="lua-language-server.exe",
    )

    download_and_install(
        repo="latex-lsp/texlab",
        asset_pattern="texlab-*-x86_64-windows.zip",
        inner_exe_pattern="texlab.exe",
        out_exe_name="texlab.exe",
    )

    print("all LSP binaries installed into", LSP_DIR)


if __name__ == "__main__":
    main()