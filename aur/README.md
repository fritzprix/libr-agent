# AUR Packaging for LibrAgent

This directory contains the necessary files to package LibrAgent for Arch Linux (AUR).

## Prerequisites

Ensure you have the base development tools installed:

```bash
sudo pacman -S --needed base-devel git
```

## Building Locally

1.  **Update Checksums**: If you modified `libragent.desktop` or want to ensure integrity, update the checksums in `PKGBUILD`.
    ```bash
    updpkgsums
    ```
    (Requires `pacman-contrib` package)

2.  **Build and Install**: Run the following command to build the package and install it:
    ```bash
    makepkg -si
    ```

## Publishing to AUR

1.  **Create an AUR Account**: Go to [aur.archlinux.org](https://aur.archlinux.org/) and create an account.
2.  **Setup SSH Keys**: Add your public SSH key to your AUR account profile.
3.  **Create Package Repository**:
    ```bash
    git clone ssh://aur@aur.archlinux.org/libragent.git
    cd libragent
    ```
4.  **Copy Files**: Copy `PKGBUILD` and `libragent.desktop` to the cloned directory.
    ```bash
    cp ../path/to/libr-agent/aur/PKGBUILD .
    cp ../path/to/libr-agent/aur/libragent.desktop .
    ```
5.  **Generate .SRCINFO**:
    ```bash
    makepkg --printsrcinfo > .SRCINFO
    ```
6.  **Commit and Push**:
    ```bash
    git add PKGBUILD .SRCINFO libragent.desktop
    git commit -m "Initial release 0.3.14"
    git push
    ```

## Notes

- The `PKGBUILD` currently pulls from the `dev/0.3.x` branch. For a stable release, you should update the `source` URL to point to a specific tag or release tarball.
- Dependencies listed are standard for Tauri v2 apps on Linux.
