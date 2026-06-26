# Guaxi Bundled Fonts

This directory is packaged through the existing `utils_bundles` flow.
During installation, `install_guaxi_fonts.ps1` registers the font files for the
current Windows user if the corresponding font family is not already installed.

Bundled font sources:

- Maple Mono NF CN: <https://github.com/subframe7536/maple-font>
- STIX Two Math: <https://ctan.org/pkg/stix2-otf>

Do not bundle proprietary system fonts such as Cambria Math. The LaTeX manual
may use Cambria Math only as a local fallback when it already exists on the
user's machine.
