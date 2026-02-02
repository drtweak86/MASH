# OS image download guidance for optional platforms

MASH willingly centers Fedora as the recommended OS for Raspberry Pi 4 installations. Still, advanced users can pursue alternative images if they need a different baseline. The following links point to the official download pages for the optional systems we currently support in documentation.

| OS | Official link | Notes |
| --- | --- | --- |
| **Ubuntu Desktop** | https://ubuntu.com/download/desktop | Current LTS (24.04) and interim releases are served from this page. |
| **Manjaro (XFCE / KDE / GNOME)** | https://manjaro.org/download/ | Downloads page that lists multiple editions plus their checksums. |
| **Raspberry Pi OS (64-bit)** | https://www.raspberrypi.com/software/ | Canonical Raspberry Pi imagery (Lite, Desktop, and Raspberry Pi OS with desktop). |

## Automation & verification
To keep these links accurate, a GitHub Actions workflow (`.github/workflows/os-download-links.yml`) runs daily and hits the URLs listed in `docs/os-download-links.toml`. If a link returns a non-success HTTP status, the workflow fails and surfaces the broken URL. Update `docs/os-download-links.toml` in lockstep whenever official download pages change.

## Fedora first
We still recommend the Fedora KDE image because it is the native platform MASH has been optimized for. Follow the steps in `docs/QUICKSTART.md` or the README for downloading and flashing Fedora. Use these optional links only when you consciously opt out of the Fedora experience.
