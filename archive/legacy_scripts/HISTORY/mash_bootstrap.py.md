# History for mash_bootstrap.py
Chosen master: **mash_bootstrap_v2_2.py** (sha e7f5fa962dad)

## Variants
- mash_bootstrap_v2_2.py (sha e7f5fa962dad)
- mash_bootstrap_v2.py (sha 748be623e486)
- mash_bootstrap.py (sha 62bb120128fd)
- mash_bootstrap(1).py (sha 085bc23203b9)
- mash_bootstrap(2).py (sha 7a8c995fca7c)

## Diffs (against master)

### mash_bootstrap_v2.py → mash_bootstrap_v2_2.py

```diff
--- mash_bootstrap_v2.py
+++ mash_bootstrap_v2_2.py
@@ -1,23 +1,33 @@
 #!/usr/bin/env python3
 """
-MASH Bootstrap (Fedora) - Master List Installer (v2)
-===================================================
-
-Fixes vs prior version:
-- Snapper: always installs `snapper` first; `snapper-plugins` is optional (only if repo has it).
-- Starship: with `--with-starship-fallback`, will install via RPM if available, otherwise installs upstream binary to /usr/local/bin.
-- Better logging for /data mount + service enable.
+MASH Bootstrap (Fedora) - Master List Installer (v2.2)
+=====================================================
+
+What this does:
+- (Optional) Enable RPM Fusion (+ tainted if requested)
+- Upgrade system
+- Install your “master list” packages grouped by category (deduped + skip already-installed)
+- Initialize Snapper on / (best-effort)
+- Force UK locale + GB keyboard
+- Ensure /data (LABEL=DATA) is mounted
+- Enable firewalld + sshd, allow mosh
+- Starship install with fallback (optional)
+- Final user QoL: ensure ~/.zshrc exists, add starship init idempotently, set default shell to zsh,
+  and nuke KDE screen lock / DPMS (best-effort)

 Run:
-  sudo ./mash_bootstrap_v2.py
-  sudo ./mash_bootstrap_v2.py --dry-run --with-starship-fallback
+  chmod +x mash_bootstrap_v2_2.py
+  sudo ./mash_bootstrap_v2_2.py --dry-run --with-starship-fallback
+  sudo ./mash_bootstrap_v2_2.py --with-starship-fallback
 """

 from __future__ import annotations

 import argparse
 import os
+import pwd
 import shlex
+import shutil
 import subprocess
 import sys
 from dataclasses import dataclass
@@ -64,7 +74,7 @@


 def dnf_pkg_available(pkg: str) -> bool:
-    """Best-effort check whether a package name exists in enabled repos."""
+    """Best-effort: returns True if repoquery finds the package."""
     try:
         r = subprocess.run(
             ["dnf", "-q", "repoquery", "--latest-limit", "1", pkg],
@@ -73,6 +83,7 @@
         )
         return r.returncode == 0
     except Exception:
+        # If repoquery isn't available for some reason, don't block installs.
         return True


@@ -91,7 +102,6 @@
     if allow_erasing:
         cmd.append("--allowerasing")
     cmd.extend(to_install)
-
     run(cmd, check=True, dry_run=dry_run)


@@ -137,6 +147,7 @@
 def setup_snapper(user: str, dry_run: bool = False) -> None:
     banner("Atomic Shield: Snapper for /")
     pkgs = ["snapper"]
+    # Plugins package name varies across distros/repos; only attempt if it exists.
     if dnf_pkg_available("snapper-plugins"):
         pkgs.append("snapper-plugins")
     dnf_install(pkgs, dry_run=dry_run)
@@ -147,6 +158,7 @@

     run(["snapper", "-c", "root", "create-config", "/"], check=False, dry_run=dry_run)
     run(["chmod", "a+rx", "/.snapshots"], check=False, dry_run=dry_run)
+    # Group-readable for your user group (best-effort)
     run(["chown", f":{user}", "/.snapshots"], check=False, dry_run=dry_run)


@@ -184,20 +196,24 @@
         subprocess.run(["findmnt", "/data"], check=False)


-def install_starship(user: str, dry_run: bool = False) -> None:
+def install_starship(dry_run: bool = False) -> None:
     if is_installed_rpm("starship"):
         print("✅ starship already installed (RPM).")
-        return
-
-    if dnf_pkg_available("starship"):
+
+    if not is_installed_rpm("starship") and dnf_pkg_available("starship"):
         banner("Starship: installing from repos")
         dnf_install(["starship"], dry_run=dry_run)
-        if is_installed_rpm("starship"):
-            return
-
-    banner("Starship: installing upstream binary to /usr/local/bin")
-    shell = "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"
-    run(["bash", "-lc", shell], check=False, dry_run=dry_run, shell_hint=shell)
+
+    if not is_installed_rpm("starship"):
+        banner("Starship: installing upstream binary to /usr/local/bin")
+        shell = "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"
+        run(["bash", "-lc", shell], check=False, dry_run=dry_run, shell_hint=shell)
+
+        # Best-effort verification
+        if dry_run:
+            print("DRY-RUN: /usr/local/bin/starship --version")
+        else:
+            subprocess.run(["/usr/local/bin/starship", "--version"], check=False)


 def enable_services(dry_run: bool) -> None:
@@ -208,6 +224,65 @@
     banner("Firewall: allow mosh (best-effort)")
     run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False, dry_run=dry_run)
     run(["firewall-cmd", "--reload"], check=False, dry_run=dry_run)
+
+
+def setup_user_qol(target_user: str, dry_run: bool = False) -> None:
+    banner(f"Final QoL: Zsh, Starship, and Power for {target_user}")
+
+    try:
+        user_info = pwd.getpwnam(target_user)
+        home_dir = user_info.pw_dir
+    except KeyError:
+        print(f"❌ User {target_user} not found. Skipping QoL steps.")
+        return
+
+    # 1) Zsh & Starship Setup (Idempotent)
+    zshrc_path = os.path.join(home_dir, ".zshrc")
+    starship_line = 'eval "$(starship init zsh)"'
+
+    if not dry_run:
+        # Ensure .zshrc exists
+        if not os.path.exists(zshrc_path):
+            with open(zshrc_path, "w", encoding="utf-8") as f:
+                f.write("# Zsh Configuration\n")
+            run(["chown", f"{target_user}:{target_user}", zshrc_path], check=False, dry_run=False)
+
+        # Check for existing line before adding
+        try:
+            with open(zshrc_path, "r", encoding="utf-8") as f:
+                content = f.read()
+        except Exception:
+            content = ""
+
+        if starship_line not in content:
+            print(f"✅ Adding Starship init to {zshrc_path}")
+            with open(zshrc_path, "a", encoding="utf-8") as f:
+                f.write(f"\n{starship_line}\n")
+
+        # Change default shell to Zsh for the user (best-effort)
+        if shutil.which("chsh") and os.path.exists("/usr/bin/zsh"):
+            run(["chsh", "-s", "/usr/bin/zsh", target_user], check=False, dry_run=False)
+        else:
+            print("⚠️ chsh or /usr/bin/zsh missing; skipping default shell change.")
+    else:
+        print(f"DRY-RUN: ensure {zshrc_path} exists + append starship init if missing")
+        print(f"DRY-RUN: chsh -s /usr/bin/zsh {target_user}")
+
+    # 2) Disable Screensaver & DPMS (KDE CLI tools) (best-effort)
+    kwrite = shutil.which("kwriteconfig6") or shutil.which("kwriteconfig5")
+    if not kwrite:
+        print("⚠️ kwriteconfig(5/6) not found; skipping KDE config tweaks.")
+    else:
+        kw = os.path.basename(kwrite)
+        kde_cmds = [
+            f"{kw} --file kscreenlockerrc --group Daemon --key Autolock false",
+            f"{kw} --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
+        ]
+        # xset only works under X11; still try (harmless if it fails).
+        kde_cmds += ["xset s off", "xset -dpms"]
+
+        for cmd in kde_cmds:
+            run(["sudo", "-u", target_user, "sh", "-c", cmd], check=False, dry_run=dry_run)


 def print_summary(categories: List[Category]) -> None:
@@ -224,7 +299,7 @@
     ensure_root()

     ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) - master list installer.")
-    ap.add_argument("--user", default="DrTweak", help="Username for /data chown and snapshot dir group (default: DrTweak)")
+    ap.add_argument("--user", default="DrTweak", help="Username for QoL steps + /data chown (default: DrTweak)")
     ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
     ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
     ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
@@ -257,7 +332,7 @@
         "bat",
         "zsh",
-        "starship",
+        # NOTE: starship handled specially below
     ]))

     categories.append(Category("System tools", [
@@ -326,37 +401,42 @@
         banner("System update: dnf upgrade --refresh")
         dnf_upgrade(dry_run=args.dry_run)

+    # Install categories
     for cat in categories:
         banner(f"Install: {cat.name}")

         if cat.name == "Media: Kodi stack":
             maybe_switch_ffmpeg(dry_run=args.dry_run)

-        if cat.name == "QoL & shell":
-            pkgs = [p for p in cat.pkgs if p != "starship"]
-            dnf_install(pkgs, dry_run=args.dry_run)
-
-            if args.with_starship_fallback:
-                install_starship(user=args.user, dry_run=args.dry_run)
-            else:
-                if dnf_pkg_available("starship"):
-                    dnf_install(["starship"], dry_run=args.dry_run)
-                else:
-                    print("⚠️ starship not found in repos; rerun with --with-starship-fallback")
-            continue
-
         dnf_install(cat.pkgs, dry_run=args.dry_run)
+
+    # Starship (special handling)
+    if args.with_starship_fallback:
+        install_starship(dry_run=args.dry_run)
+    else:
+        # Only attempt via RPM if it exists; otherwise tell you what to do.
+        if dnf_pkg_available("starship"):
+            banner("Starship: installing from repos")
+            dnf_install(["starship"], dry_run=args.dry_run)
+        else:
+            banner("Starship")
+            print("⚠️ starship not found in repos; rerun with --with-starship-fallback")

     if not args.no_data_mount:
         mount_data_partition(user=args.user, dry_run=args.dry_run)

     enable_services(dry_run=args.dry_run)

-    banner("DONE")
-    print("Oh My Zsh (not an RPM):")
+    # Final QoL hooks (zshrc, starship init, screensaver nuke)
+    setup_user_qol(target_user=args.user, dry_run=args.dry_run)
+
+    banner("DONE - SCOOT BOOGIE COMPLETE")
+    print(f"✅ 4TB Fedora System configured for {args.user}.")
+    print("🚀 Log out and back in (or reboot) to see your new setup!")
+    print("\nOh My Zsh (not an RPM):")
     print('  sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)"')
     print("Starship init (zsh):")
-    print('  echo 'eval "$(starship init zsh)"' >> ~/.zshrc')
+    print('  echo \'eval "$(starship init zsh)"\' >> ~/.zshrc')
     print("If libdvdcss wasn't found: rerun with --with-tainted")
     return 0

```

### mash_bootstrap.py → mash_bootstrap_v2_2.py

```diff
--- mash_bootstrap.py
+++ mash_bootstrap_v2_2.py
@@ -1,41 +1,53 @@
 #!/usr/bin/env python3
 """
-MASH Bootstrap (Fedora) — Master List Installer
-==============================================
-
-Goals:
-- Category-based, deduped package list.
-- Idempotent: skips packages already installed.
-- Fedora-native: uses dnf (installs weak deps / "recommended" by default).
-- Robust: skips unavailable packages, can enable RPM Fusion (+ tainted), can handle ffmpeg swap.
+MASH Bootstrap (Fedora) - Master List Installer (v2.2)
+=====================================================
+
+What this does:
+- (Optional) Enable RPM Fusion (+ tainted if requested)
+- Upgrade system
+- Install your “master list” packages grouped by category (deduped + skip already-installed)
+- Initialize Snapper on / (best-effort)
+- Force UK locale + GB keyboard
+- Ensure /data (LABEL=DATA) is mounted
+- Enable firewalld + sshd, allow mosh
+- Starship install with fallback (optional)
+- Final user QoL: ensure ~/.zshrc exists, add starship init idempotently, set default shell to zsh,
+  and nuke KDE screen lock / DPMS (best-effort)

 Run:
-  sudo ./mash_bootstrap.py
-
-Common options:
-  sudo ./mash_bootstrap.py --with-kodi --with-tainted
-  sudo ./mash_bootstrap.py --dry-run
+  chmod +x mash_bootstrap_v2_2.py
+  sudo ./mash_bootstrap_v2_2.py --dry-run --with-starship-fallback
+  sudo ./mash_bootstrap_v2_2.py --with-starship-fallback
 """

 from __future__ import annotations

 import argparse
 import os
+import pwd
 import shlex
+import shutil
 import subprocess
 import sys
 from dataclasses import dataclass
 from typing import List, Set


-def sh(cmd: List[str], check: bool = True) -> subprocess.CompletedProcess:
-    return subprocess.run(cmd, check=check)
-
-
 def banner(msg: str) -> None:
     print("\n" + "=" * 80)
     print(msg)
     print("=" * 80)
+
+
+def run(cmd: List[str], *, check: bool = True, dry_run: bool = False, shell_hint: str | None = None) -> None:
+    if dry_run:
+        if shell_hint:
+            print("DRY-RUN:", shell_hint)
+        else:
+            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
+        return
+    subprocess.run(cmd, check=check)


 def is_installed_rpm(pkg: str) -> bool:
@@ -61,6 +73,20 @@
     pkgs: List[str]


+def dnf_pkg_available(pkg: str) -> bool:
+    """Best-effort: returns True if repoquery finds the package."""
+    try:
+        r = subprocess.run(
+            ["dnf", "-q", "repoquery", "--latest-limit", "1", pkg],
+            stdout=subprocess.DEVNULL,
+            stderr=subprocess.DEVNULL,
+        )
+        return r.returncode == 0
+    except Exception:
+        # If repoquery isn't available for some reason, don't block installs.
+        return True
+
+
 def dnf_install(pkgs: List[str], *, allow_erasing: bool = False, dry_run: bool = False) -> None:
     pkgs = dedupe_keep_order(pkgs)
     to_install = [p for p in pkgs if not is_installed_rpm(p)]
@@ -76,20 +102,11 @@
     if allow_erasing:
         cmd.append("--allowerasing")
     cmd.extend(to_install)
-
-    if dry_run:
-        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        return
-
-    sh(cmd, check=True)
+    run(cmd, check=True, dry_run=dry_run)


 def dnf_upgrade(*, dry_run: bool = False) -> None:
-    cmd = ["dnf", "upgrade", "--refresh", "-y"]
-    if dry_run:
-        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        return
-    sh(cmd, check=True)
+    run(["dnf", "upgrade", "--refresh", "-y"], check=True, dry_run=dry_run)


 def ensure_root() -> None:
@@ -111,11 +128,7 @@
             f"https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-{fed}.noarch.rpm",
             f"https://download1.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-{fed}.noarch.rpm",
         ]
-        cmd = ["dnf", "install", "-y"] + urls
-        if dry_run:
-            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        else:
-            sh(cmd, check=True)
+        run(["dnf", "install", "-y", *urls], check=True, dry_run=dry_run)

     if with_tainted:
         banner("Repos: RPM Fusion tainted (for libdvdcss etc.)")
@@ -131,24 +144,145 @@
         print("✅ ffmpeg swap not needed.")


+def setup_snapper(user: str, dry_run: bool = False) -> None:
+    banner("Atomic Shield: Snapper for /")
+    pkgs = ["snapper"]
+    # Plugins package name varies across distros/repos; only attempt if it exists.
+    if dnf_pkg_available("snapper-plugins"):
+        pkgs.append("snapper-plugins")
+    dnf_install(pkgs, dry_run=dry_run)
+
+    if not is_installed_rpm("snapper"):
+        print("⚠️ snapper not installed; skipping init.")
+        return
+
+    run(["snapper", "-c", "root", "create-config", "/"], check=False, dry_run=dry_run)
+    run(["chmod", "a+rx", "/.snapshots"], check=False, dry_run=dry_run)
+    # Group-readable for your user group (best-effort)
+    run(["chown", f":{user}", "/.snapshots"], check=False, dry_run=dry_run)
+
+
+def setup_uk_locale(dry_run: bool = False) -> None:
+    banner("Locale: en_GB + GB keyboard")
+    run(["dnf", "install", "-y", "langpacks-en_GB"], check=False, dry_run=dry_run)
+    run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False, dry_run=dry_run)
+    run(["localectl", "set-x11-keymap", "gb"], check=False, dry_run=dry_run)
+
+
+def mount_data_partition(user: str, dry_run: bool = False) -> None:
+    banner("Storage: ensure DATA partition mounted at /data")
+    fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"
+
+    run(["mkdir", "-p", "/data"], check=False, dry_run=dry_run)
+
+    if dry_run:
+        print("DRY-RUN: append fstab line if missing:", fstab_line.strip())
+    else:
+        try:
+            with open("/etc/fstab", "r", encoding="utf-8") as f:
+                txt = f.read()
+        except FileNotFoundError:
+            txt = ""
+        if "/data" not in txt:
+            with open("/etc/fstab", "a", encoding="utf-8") as f:
+                f.write(fstab_line)
+
+    run(["mount", "-a"], check=False, dry_run=dry_run)
+    run(["chown", f"{user}:{user}", "/data"], check=False, dry_run=dry_run)
+
+    if dry_run:
+        print("DRY-RUN: verify mountpoint: findmnt /data")
+    else:
+        subprocess.run(["findmnt", "/data"], check=False)
+
+
+def install_starship(dry_run: bool = False) -> None:
+    if is_installed_rpm("starship"):
+        print("✅ starship already installed (RPM).")
+
+    if not is_installed_rpm("starship") and dnf_pkg_available("starship"):
+        banner("Starship: installing from repos")
+        dnf_install(["starship"], dry_run=dry_run)
+
+    if not is_installed_rpm("starship"):
+        banner("Starship: installing upstream binary to /usr/local/bin")
+        shell = "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"
+        run(["bash", "-lc", shell], check=False, dry_run=dry_run, shell_hint=shell)
+
+        # Best-effort verification
+        if dry_run:
+            print("DRY-RUN: /usr/local/bin/starship --version")
+        else:
+            subprocess.run(["/usr/local/bin/starship", "--version"], check=False)
+
+
 def enable_services(dry_run: bool) -> None:
     banner("Services: firewalld + sshd")
-    cmds = [
-        ["systemctl", "enable", "--now", "firewalld"],
-        ["systemctl", "enable", "--now", "sshd"],
-    ]
-    for cmd in cmds:
-        if dry_run:
-            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
+    run(["systemctl", "enable", "--now", "firewalld"], check=False, dry_run=dry_run)
+    run(["systemctl", "enable", "--now", "sshd"], check=False, dry_run=dry_run)
+
+    banner("Firewall: allow mosh (best-effort)")
+    run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False, dry_run=dry_run)
+    run(["firewall-cmd", "--reload"], check=False, dry_run=dry_run)
+
+
+def setup_user_qol(target_user: str, dry_run: bool = False) -> None:
+    banner(f"Final QoL: Zsh, Starship, and Power for {target_user}")
+
+    try:
+        user_info = pwd.getpwnam(target_user)
+        home_dir = user_info.pw_dir
+    except KeyError:
+        print(f"❌ User {target_user} not found. Skipping QoL steps.")
+        return
+
+    # 1) Zsh & Starship Setup (Idempotent)
+    zshrc_path = os.path.join(home_dir, ".zshrc")
+    starship_line = 'eval "$(starship init zsh)"'
+
+    if not dry_run:
+        # Ensure .zshrc exists
+        if not os.path.exists(zshrc_path):
+            with open(zshrc_path, "w", encoding="utf-8") as f:
+                f.write("# Zsh Configuration\n")
+            run(["chown", f"{target_user}:{target_user}", zshrc_path], check=False, dry_run=False)
+
+        # Check for existing line before adding
+        try:
+            with open(zshrc_path, "r", encoding="utf-8") as f:
+                content = f.read()
+        except Exception:
+            content = ""
+
+        if starship_line not in content:
+            print(f"✅ Adding Starship init to {zshrc_path}")
+            with open(zshrc_path, "a", encoding="utf-8") as f:
+                f.write(f"\n{starship_line}\n")
+
+        # Change default shell to Zsh for the user (best-effort)
+        if shutil.which("chsh") and os.path.exists("/usr/bin/zsh"):
+            run(["chsh", "-s", "/usr/bin/zsh", target_user], check=False, dry_run=False)
         else:
-            subprocess.run(cmd, check=False)
-
-    banner("Firewall: allow mosh (best-effort)")
-    if dry_run:
-        print("DRY-RUN: firewall-cmd --permanent --add-service=mosh && firewall-cmd --reload")
-        return
-    subprocess.run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False)
-    subprocess.run(["firewall-cmd", "--reload"], check=False)
+            print("⚠️ chsh or /usr/bin/zsh missing; skipping default shell change.")
+    else:
+        print(f"DRY-RUN: ensure {zshrc_path} exists + append starship init if missing")
+        print(f"DRY-RUN: chsh -s /usr/bin/zsh {target_user}")
+
+    # 2) Disable Screensaver & DPMS (KDE CLI tools) (best-effort)
+    kwrite = shutil.which("kwriteconfig6") or shutil.which("kwriteconfig5")
+    if not kwrite:
+        print("⚠️ kwriteconfig(5/6) not found; skipping KDE config tweaks.")
+    else:
+        kw = os.path.basename(kwrite)
+        kde_cmds = [
+            f"{kw} --file kscreenlockerrc --group Daemon --key Autolock false",
+            f"{kw} --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
+        ]
+        # xset only works under X11; still try (harmless if it fails).
+        kde_cmds += ["xset s off", "xset -dpms"]
+
+        for cmd in kde_cmds:
+            run(["sudo", "-u", target_user, "sh", "-c", cmd], check=False, dry_run=dry_run)


 def print_summary(categories: List[Category]) -> None:
@@ -164,12 +298,16 @@
 def main() -> int:
     ensure_root()

-    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) — master list installer.")
+    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) - master list installer.")
+    ap.add_argument("--user", default="DrTweak", help="Username for QoL steps + /data chown (default: DrTweak)")
     ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
     ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
     ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
     ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
-    ap.add_argument("--with-snapper", action="store_true", help="Install snapper + plugins (does not auto-configure)")
+    ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper early")
+    ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force UK locale/keyboard")
+    ap.add_argument("--no-data-mount", action="store_true", help="Do NOT setup /data automount")
+    ap.add_argument("--with-starship-fallback", action="store_true", help="Install starship via upstream if RPM missing")
     ap.add_argument("--dry-run", action="store_true", help="Print commands but do not execute")
     args = ap.parse_args()

@@ -181,7 +319,7 @@
     ]))

     categories.append(Category("Dev & build", [
-        "git", "cmake", "ninja",
+        "git", "cmake", "ninja-build",
         "gcc", "gcc-c++", "ccache",
         "pkgconf", "autoconf", "automake", "libtool",
         "python3-devel", "patchelf",
@@ -193,7 +331,12 @@
         "unzip", "zip", "tree",
         "bat",
-        "zsh", "starship",
+        "zsh",
+        # NOTE: starship handled specially below
+    ]))
+
+    categories.append(Category("System tools", [
+        "btop", "btrfs-assistant", "nvme-cli",
     ]))

     categories.append(Category("Networking", [
@@ -223,14 +366,14 @@
         "waylandpp",
     ]))

+    categories.append(Category("Multimedia codecs", [
+        "gstreamer1-plugins-ugly",
+        "gstreamer1-plugins-bad-free-extras",
+    ]))
+
     categories.append(Category("Database client libs", [
         "mariadb-connector-c",
     ]))
-
-    if args.with_snapper:
-        categories.append(Category("Btrfs snapshots (snapper)", [
-            "snapper", "snapper-plugins",
-        ]))

     if args.with_kodi:
         categories.append(Category("Media: Kodi stack", [
@@ -245,6 +388,12 @@

     print_summary(categories)

+    if not args.no_snapper_init:
+        setup_snapper(user=args.user, dry_run=args.dry_run)
+
+    if not args.no_uk_locale:
+        setup_uk_locale(dry_run=args.dry_run)
+
     if not args.no_rpmfusion:
         enable_rpmfusion(with_tainted=args.with_tainted, dry_run=args.dry_run)

@@ -252,21 +401,43 @@
         banner("System update: dnf upgrade --refresh")
         dnf_upgrade(dry_run=args.dry_run)

+    # Install categories
     for cat in categories:
         banner(f"Install: {cat.name}")
+
         if cat.name == "Media: Kodi stack":
             maybe_switch_ffmpeg(dry_run=args.dry_run)
+
         dnf_install(cat.pkgs, dry_run=args.dry_run)

+    # Starship (special handling)
+    if args.with_starship_fallback:
+        install_starship(dry_run=args.dry_run)
+    else:
+        # Only attempt via RPM if it exists; otherwise tell you what to do.
+        if dnf_pkg_available("starship"):
+            banner("Starship: installing from repos")
+            dnf_install(["starship"], dry_run=args.dry_run)
+        else:
+            banner("Starship")
+            print("⚠️ starship not found in repos; rerun with --with-starship-fallback")
+
+    if not args.no_data_mount:
+        mount_data_partition(user=args.user, dry_run=args.dry_run)
+
     enable_services(dry_run=args.dry_run)

-    banner("DONE")
-    print("Oh My Zsh (not an RPM):")
-    print("  sh -c \"$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)\"")
+    # Final QoL hooks (zshrc, starship init, screensaver nuke)
... (diff truncated) ...
```

### mash_bootstrap(1).py → mash_bootstrap_v2_2.py

```diff
--- mash_bootstrap(1).py
+++ mash_bootstrap_v2_2.py
@@ -1,41 +1,53 @@
 #!/usr/bin/env python3
 """
-MASH Bootstrap (Fedora) — Master List Installer
-==============================================
-
-Goals:
-- Category-based, deduped package list.
-- Idempotent: skips packages already installed.
-- Fedora-native: uses dnf (installs weak deps / "recommended" by default).
-- Robust: skips unavailable packages, can enable RPM Fusion (+ tainted), can handle ffmpeg swap.
+MASH Bootstrap (Fedora) - Master List Installer (v2.2)
+=====================================================
+
+What this does:
+- (Optional) Enable RPM Fusion (+ tainted if requested)
+- Upgrade system
+- Install your “master list” packages grouped by category (deduped + skip already-installed)
+- Initialize Snapper on / (best-effort)
+- Force UK locale + GB keyboard
+- Ensure /data (LABEL=DATA) is mounted
+- Enable firewalld + sshd, allow mosh
+- Starship install with fallback (optional)
+- Final user QoL: ensure ~/.zshrc exists, add starship init idempotently, set default shell to zsh,
+  and nuke KDE screen lock / DPMS (best-effort)

 Run:
-  sudo ./mash_bootstrap.py
-
-Common options:
-  sudo ./mash_bootstrap.py --with-kodi --with-tainted
-  sudo ./mash_bootstrap.py --dry-run
+  chmod +x mash_bootstrap_v2_2.py
+  sudo ./mash_bootstrap_v2_2.py --dry-run --with-starship-fallback
+  sudo ./mash_bootstrap_v2_2.py --with-starship-fallback
 """

 from __future__ import annotations

 import argparse
 import os
+import pwd
 import shlex
+import shutil
 import subprocess
 import sys
 from dataclasses import dataclass
 from typing import List, Set


-def sh(cmd: List[str], check: bool = True) -> subprocess.CompletedProcess:
-    return subprocess.run(cmd, check=check)
-
-
 def banner(msg: str) -> None:
     print("\n" + "=" * 80)
     print(msg)
     print("=" * 80)
+
+
+def run(cmd: List[str], *, check: bool = True, dry_run: bool = False, shell_hint: str | None = None) -> None:
+    if dry_run:
+        if shell_hint:
+            print("DRY-RUN:", shell_hint)
+        else:
+            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
+        return
+    subprocess.run(cmd, check=check)


 def is_installed_rpm(pkg: str) -> bool:
@@ -61,6 +73,20 @@
     pkgs: List[str]


+def dnf_pkg_available(pkg: str) -> bool:
+    """Best-effort: returns True if repoquery finds the package."""
+    try:
+        r = subprocess.run(
+            ["dnf", "-q", "repoquery", "--latest-limit", "1", pkg],
+            stdout=subprocess.DEVNULL,
+            stderr=subprocess.DEVNULL,
+        )
+        return r.returncode == 0
+    except Exception:
+        # If repoquery isn't available for some reason, don't block installs.
+        return True
+
+
 def dnf_install(pkgs: List[str], *, allow_erasing: bool = False, dry_run: bool = False) -> None:
     pkgs = dedupe_keep_order(pkgs)
     to_install = [p for p in pkgs if not is_installed_rpm(p)]
@@ -76,20 +102,11 @@
     if allow_erasing:
         cmd.append("--allowerasing")
     cmd.extend(to_install)
-
-    if dry_run:
-        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        return
-
-    sh(cmd, check=True)
+    run(cmd, check=True, dry_run=dry_run)


 def dnf_upgrade(*, dry_run: bool = False) -> None:
-    cmd = ["dnf", "upgrade", "--refresh", "-y"]
-    if dry_run:
-        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        return
-    sh(cmd, check=True)
+    run(["dnf", "upgrade", "--refresh", "-y"], check=True, dry_run=dry_run)


 def ensure_root() -> None:
@@ -111,86 +128,11 @@
             f"https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-{fed}.noarch.rpm",
             f"https://download1.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-{fed}.noarch.rpm",
         ]
-        cmd = ["dnf", "install", "-y"] + urls
-        if dry_run:
-            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        else:
-            sh(cmd, check=True)
+        run(["dnf", "install", "-y", *urls], check=True, dry_run=dry_run)

     if with_tainted:
         banner("Repos: RPM Fusion tainted (for libdvdcss etc.)")
         dnf_install(["rpmfusion-free-release-tainted", "rpmfusion-nonfree-release-tainted"], dry_run=dry_run)
-
-
-
-def setup_snapper(username: str, dry_run: bool = False) -> None:
-    """
-    Atomic shield: ensure snapper is installed and initialize snapshots for / early,
-    so you can roll back if anything goes sideways mid-bootstrapping.
-    """
-    banner("Atomic Shield: Snapper for /")
-    # Ensure snapper is present first (idempotent)
-    dnf_install(["snapper", "snapper-plugins"], dry_run=dry_run)
-
-    if dry_run:
-        print("DRY-RUN: snapper -c root create-config /")
-        print("DRY-RUN: chmod a+rx /.snapshots")
-        print(f"DRY-RUN: chown :{shlex.quote(username)} /.snapshots  (best-effort)")
-        return
-
-    # Create config (safe if already exists)
-    subprocess.run(["snapper", "-c", "root", "create-config", "/"], check=False)
-
-    # Make snapshots dir browsable and allow your user group to access (best-effort)
-    subprocess.run(["chmod", "a+rx", "/.snapshots"], check=False)
-    subprocess.run(["chown", f":{username}", "/.snapshots"], check=False)
-
-
-def setup_uk_locale(dry_run: bool = False) -> None:
-    """
-    Proper English: force UK locale + keyboard.
-    """
-    banner("Locale: en_GB + GB keyboard")
-    if dry_run:
-        print("DRY-RUN: dnf install -y langpacks-en_GB")
-        print("DRY-RUN: localectl set-locale LANG=en_GB.UTF-8")
-        print("DRY-RUN: localectl set-x11-keymap gb")
-        return
-
-    subprocess.run(["dnf", "install", "-y", "langpacks-en_GB"], check=False)
-    subprocess.run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False)
-    subprocess.run(["localectl", "set-x11-keymap", "gb"], check=False)
-
-
-def mount_data_partition(username: str, dry_run: bool = False) -> None:
-    """
-    Ensure the 1.9TiB DATA partition (LABEL=DATA) is mounted at /data and owned by the user.
-    """
-    banner("Storage: ensure DATA partition mounted at /data")
-    fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"
-
-    if dry_run:
-        print("DRY-RUN: mkdir -p /data")
-        print("DRY-RUN: append fstab line if missing:", fstab_line.strip())
-        print("DRY-RUN: mount -a")
-        print(f"DRY-RUN: chown {shlex.quote(username)}:{shlex.quote(username)} /data")
-        return
-
-    os.makedirs("/data", exist_ok=True)
-
-    # Append to /etc/fstab only if it's not already present (by mountpoint or label)
-    try:
-        with open("/etc/fstab", "r", encoding="utf-8") as f:
-            fstab = f.read()
-    except Exception:
-        fstab = ""
-
-    if ("/data" not in fstab) and ("LABEL=DATA" not in fstab):
-        with open("/etc/fstab", "a", encoding="utf-8") as f_append:
-            f_append.write(fstab_line)
-
-    subprocess.run(["mount", "-a"], check=False)
-    subprocess.run(["chown", f"{username}:{username}", "/data"], check=False)


 def maybe_switch_ffmpeg(dry_run: bool) -> None:
@@ -202,24 +144,145 @@
         print("✅ ffmpeg swap not needed.")


+def setup_snapper(user: str, dry_run: bool = False) -> None:
+    banner("Atomic Shield: Snapper for /")
+    pkgs = ["snapper"]
+    # Plugins package name varies across distros/repos; only attempt if it exists.
+    if dnf_pkg_available("snapper-plugins"):
+        pkgs.append("snapper-plugins")
+    dnf_install(pkgs, dry_run=dry_run)
+
+    if not is_installed_rpm("snapper"):
+        print("⚠️ snapper not installed; skipping init.")
+        return
+
+    run(["snapper", "-c", "root", "create-config", "/"], check=False, dry_run=dry_run)
+    run(["chmod", "a+rx", "/.snapshots"], check=False, dry_run=dry_run)
+    # Group-readable for your user group (best-effort)
+    run(["chown", f":{user}", "/.snapshots"], check=False, dry_run=dry_run)
+
+
+def setup_uk_locale(dry_run: bool = False) -> None:
+    banner("Locale: en_GB + GB keyboard")
+    run(["dnf", "install", "-y", "langpacks-en_GB"], check=False, dry_run=dry_run)
+    run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False, dry_run=dry_run)
+    run(["localectl", "set-x11-keymap", "gb"], check=False, dry_run=dry_run)
+
+
+def mount_data_partition(user: str, dry_run: bool = False) -> None:
+    banner("Storage: ensure DATA partition mounted at /data")
+    fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"
+
+    run(["mkdir", "-p", "/data"], check=False, dry_run=dry_run)
+
+    if dry_run:
+        print("DRY-RUN: append fstab line if missing:", fstab_line.strip())
+    else:
+        try:
+            with open("/etc/fstab", "r", encoding="utf-8") as f:
+                txt = f.read()
+        except FileNotFoundError:
+            txt = ""
+        if "/data" not in txt:
+            with open("/etc/fstab", "a", encoding="utf-8") as f:
+                f.write(fstab_line)
+
+    run(["mount", "-a"], check=False, dry_run=dry_run)
+    run(["chown", f"{user}:{user}", "/data"], check=False, dry_run=dry_run)
+
+    if dry_run:
+        print("DRY-RUN: verify mountpoint: findmnt /data")
+    else:
+        subprocess.run(["findmnt", "/data"], check=False)
+
+
+def install_starship(dry_run: bool = False) -> None:
+    if is_installed_rpm("starship"):
+        print("✅ starship already installed (RPM).")
+
+    if not is_installed_rpm("starship") and dnf_pkg_available("starship"):
+        banner("Starship: installing from repos")
+        dnf_install(["starship"], dry_run=dry_run)
+
+    if not is_installed_rpm("starship"):
+        banner("Starship: installing upstream binary to /usr/local/bin")
+        shell = "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"
+        run(["bash", "-lc", shell], check=False, dry_run=dry_run, shell_hint=shell)
+
+        # Best-effort verification
+        if dry_run:
+            print("DRY-RUN: /usr/local/bin/starship --version")
+        else:
+            subprocess.run(["/usr/local/bin/starship", "--version"], check=False)
+
+
 def enable_services(dry_run: bool) -> None:
     banner("Services: firewalld + sshd")
-    cmds = [
-        ["systemctl", "enable", "--now", "firewalld"],
-        ["systemctl", "enable", "--now", "sshd"],
-    ]
-    for cmd in cmds:
-        if dry_run:
-            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
+    run(["systemctl", "enable", "--now", "firewalld"], check=False, dry_run=dry_run)
+    run(["systemctl", "enable", "--now", "sshd"], check=False, dry_run=dry_run)
+
+    banner("Firewall: allow mosh (best-effort)")
+    run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False, dry_run=dry_run)
+    run(["firewall-cmd", "--reload"], check=False, dry_run=dry_run)
+
+
+def setup_user_qol(target_user: str, dry_run: bool = False) -> None:
+    banner(f"Final QoL: Zsh, Starship, and Power for {target_user}")
+
+    try:
+        user_info = pwd.getpwnam(target_user)
+        home_dir = user_info.pw_dir
+    except KeyError:
+        print(f"❌ User {target_user} not found. Skipping QoL steps.")
+        return
+
+    # 1) Zsh & Starship Setup (Idempotent)
+    zshrc_path = os.path.join(home_dir, ".zshrc")
+    starship_line = 'eval "$(starship init zsh)"'
+
+    if not dry_run:
+        # Ensure .zshrc exists
+        if not os.path.exists(zshrc_path):
+            with open(zshrc_path, "w", encoding="utf-8") as f:
+                f.write("# Zsh Configuration\n")
+            run(["chown", f"{target_user}:{target_user}", zshrc_path], check=False, dry_run=False)
+
+        # Check for existing line before adding
+        try:
+            with open(zshrc_path, "r", encoding="utf-8") as f:
+                content = f.read()
+        except Exception:
+            content = ""
+
+        if starship_line not in content:
+            print(f"✅ Adding Starship init to {zshrc_path}")
+            with open(zshrc_path, "a", encoding="utf-8") as f:
+                f.write(f"\n{starship_line}\n")
+
+        # Change default shell to Zsh for the user (best-effort)
+        if shutil.which("chsh") and os.path.exists("/usr/bin/zsh"):
+            run(["chsh", "-s", "/usr/bin/zsh", target_user], check=False, dry_run=False)
         else:
-            subprocess.run(cmd, check=False)
-
-    banner("Firewall: allow mosh (best-effort)")
-    if dry_run:
-        print("DRY-RUN: firewall-cmd --permanent --add-service=mosh && firewall-cmd --reload")
-        return
-    subprocess.run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False)
-    subprocess.run(["firewall-cmd", "--reload"], check=False)
+            print("⚠️ chsh or /usr/bin/zsh missing; skipping default shell change.")
+    else:
+        print(f"DRY-RUN: ensure {zshrc_path} exists + append starship init if missing")
+        print(f"DRY-RUN: chsh -s /usr/bin/zsh {target_user}")
+
+    # 2) Disable Screensaver & DPMS (KDE CLI tools) (best-effort)
+    kwrite = shutil.which("kwriteconfig6") or shutil.which("kwriteconfig5")
+    if not kwrite:
+        print("⚠️ kwriteconfig(5/6) not found; skipping KDE config tweaks.")
+    else:
+        kw = os.path.basename(kwrite)
+        kde_cmds = [
+            f"{kw} --file kscreenlockerrc --group Daemon --key Autolock false",
+            f"{kw} --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
+        ]
+        # xset only works under X11; still try (harmless if it fails).
+        kde_cmds += ["xset s off", "xset -dpms"]
+
+        for cmd in kde_cmds:
+            run(["sudo", "-u", target_user, "sh", "-c", cmd], check=False, dry_run=dry_run)


 def print_summary(categories: List[Category]) -> None:
@@ -235,16 +298,16 @@
 def main() -> int:
     ensure_root()

-    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) — master list installer.")
-    ap.add_argument("--user", default="DrTweak", help="Primary username/group for ownership (default: DrTweak)")
+    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) - master list installer.")
+    ap.add_argument("--user", default="DrTweak", help="Username for QoL steps + /data chown (default: DrTweak)")
     ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
     ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
     ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
     ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
-    ap.add_argument("--with-snapper", action="store_true", help="Install snapper + plugins (does not auto-configure)")
-    ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper for / early")
-    ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force en_GB locale + GB keymap")
-    ap.add_argument("--no-data-mount", action="store_true", help="Do NOT ensure LABEL=DATA is mounted at /data")
+    ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper early")
+    ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force UK locale/keyboard")
+    ap.add_argument("--no-data-mount", action="store_true", help="Do NOT setup /data automount")
+    ap.add_argument("--with-starship-fallback", action="store_true", help="Install starship via upstream if RPM missing")
     ap.add_argument("--dry-run", action="store_true", help="Print commands but do not execute")
     args = ap.parse_args()

@@ -256,7 +319,7 @@
     ]))

     categories.append(Category("Dev & build", [
-        "git", "cmake", "ninja",
+        "git", "cmake", "ninja-build",
         "gcc", "gcc-c++", "ccache",
         "pkgconf", "autoconf", "automake", "libtool",
         "python3-devel", "patchelf",
@@ -268,13 +331,12 @@
         "unzip", "zip", "tree",
... (diff truncated) ...
```

### mash_bootstrap(2).py → mash_bootstrap_v2_2.py

```diff
--- mash_bootstrap(2).py
+++ mash_bootstrap_v2_2.py
@@ -1,45 +1,53 @@
 #!/usr/bin/env python3
 """
-MASH Bootstrap (Fedora) — Master List Installer
-==============================================
-
-Goals:
-- Category-based, deduped package list.
-- Idempotent: skips packages already installed.
-- Fedora-native: uses dnf and keeps weak deps ("recommended") enabled.
-- Robust: skips unavailable packages, can enable RPM Fusion (+ tainted), can handle ffmpeg swap.
-- Scoot Boogie extras:
-  - Initialize Snapper early (atomic shield).
-  - Force UK locale + keyboard.
-  - Ensure 4TB DATA partition mounts at /data.
+MASH Bootstrap (Fedora) - Master List Installer (v2.2)
+=====================================================
+
+What this does:
+- (Optional) Enable RPM Fusion (+ tainted if requested)
+- Upgrade system
+- Install your “master list” packages grouped by category (deduped + skip already-installed)
+- Initialize Snapper on / (best-effort)
+- Force UK locale + GB keyboard
+- Ensure /data (LABEL=DATA) is mounted
+- Enable firewalld + sshd, allow mosh
+- Starship install with fallback (optional)
+- Final user QoL: ensure ~/.zshrc exists, add starship init idempotently, set default shell to zsh,
+  and nuke KDE screen lock / DPMS (best-effort)

 Run:
-  sudo ./mash_bootstrap.py
-
-Common:
-  sudo ./mash_bootstrap.py --dry-run
-  sudo ./mash_bootstrap.py --with-kodi --with-tainted
+  chmod +x mash_bootstrap_v2_2.py
+  sudo ./mash_bootstrap_v2_2.py --dry-run --with-starship-fallback
+  sudo ./mash_bootstrap_v2_2.py --with-starship-fallback
 """

 from __future__ import annotations

 import argparse
 import os
+import pwd
 import shlex
+import shutil
 import subprocess
 import sys
 from dataclasses import dataclass
 from typing import List, Set


-def sh(cmd: List[str], check: bool = True) -> subprocess.CompletedProcess:
-    return subprocess.run(cmd, check=check)
-
-
 def banner(msg: str) -> None:
     print("\n" + "=" * 80)
     print(msg)
     print("=" * 80)
+
+
+def run(cmd: List[str], *, check: bool = True, dry_run: bool = False, shell_hint: str | None = None) -> None:
+    if dry_run:
+        if shell_hint:
+            print("DRY-RUN:", shell_hint)
+        else:
+            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
+        return
+    subprocess.run(cmd, check=check)


 def is_installed_rpm(pkg: str) -> bool:
@@ -66,8 +74,7 @@


 def dnf_pkg_available(pkg: str) -> bool:
-    """Best-effort check whether *a package name* exists in enabled repos."""
-    # dnf5 usually has repoquery; if not, we just return True and let --skip-unavailable handle it.
+    """Best-effort: returns True if repoquery finds the package."""
     try:
         r = subprocess.run(
             ["dnf", "-q", "repoquery", "--latest-limit", "1", pkg],
@@ -75,16 +82,13 @@
             stderr=subprocess.DEVNULL,
         )
         return r.returncode == 0
-    except FileNotFoundError:
-        return True
     except Exception:
+        # If repoquery isn't available for some reason, don't block installs.
         return True


 def dnf_install(pkgs: List[str], *, allow_erasing: bool = False, dry_run: bool = False) -> None:
     pkgs = dedupe_keep_order(pkgs)
-
-    # Filter already-installed to avoid noisy "already installed" transaction failures on some setups.
     to_install = [p for p in pkgs if not is_installed_rpm(p)]
     if not to_install:
         print("✅ Nothing new to install in this step.")
@@ -98,20 +102,11 @@
     if allow_erasing:
         cmd.append("--allowerasing")
     cmd.extend(to_install)
-
-    if dry_run:
-        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        return
-
-    sh(cmd, check=True)
+    run(cmd, check=True, dry_run=dry_run)


 def dnf_upgrade(*, dry_run: bool = False) -> None:
-    cmd = ["dnf", "upgrade", "--refresh", "-y"]
-    if dry_run:
-        print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        return
-    sh(cmd, check=True)
+    run(["dnf", "upgrade", "--refresh", "-y"], check=True, dry_run=dry_run)


 def ensure_root() -> None:
@@ -133,11 +128,7 @@
             f"https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-{fed}.noarch.rpm",
             f"https://download1.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-{fed}.noarch.rpm",
         ]
-        cmd = ["dnf", "install", "-y"] + urls
-        if dry_run:
-            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
-        else:
-            sh(cmd, check=True)
+        run(["dnf", "install", "-y", *urls], check=True, dry_run=dry_run)

     if with_tainted:
         banner("Repos: RPM Fusion tainted (for libdvdcss etc.)")
@@ -154,98 +145,144 @@


 def setup_snapper(user: str, dry_run: bool = False) -> None:
-    """Atomic Shield: install + initialize snapper ASAP."""
     banner("Atomic Shield: Snapper for /")
-    # Fedora 43 doesn't ship snapper-plugins; keep it optional if it exists in repos.
     pkgs = ["snapper"]
+    # Plugins package name varies across distros/repos; only attempt if it exists.
     if dnf_pkg_available("snapper-plugins"):
         pkgs.append("snapper-plugins")
-
     dnf_install(pkgs, dry_run=dry_run)

-    # If snapper now installed, init config best-effort.
     if not is_installed_rpm("snapper"):
         print("⚠️ snapper not installed; skipping init.")
         return

-    if dry_run:
-        print("DRY-RUN: snapper -c root create-config /")
-        print("DRY-RUN: chmod a+rx /.snapshots")
-        print(f"DRY-RUN: chown :{user} /.snapshots  (best-effort)")
-        return
-
-    subprocess.run(["snapper", "-c", "root", "create-config", "/"], check=False)
-    subprocess.run(["chmod", "a+rx", "/.snapshots"], check=False)
-    # This expects a group with the same name as the user; if it doesn't exist, it's harmless.
-    subprocess.run(["chown", f":{user}", "/.snapshots"], check=False)
+    run(["snapper", "-c", "root", "create-config", "/"], check=False, dry_run=dry_run)
+    run(["chmod", "a+rx", "/.snapshots"], check=False, dry_run=dry_run)
+    # Group-readable for your user group (best-effort)
+    run(["chown", f":{user}", "/.snapshots"], check=False, dry_run=dry_run)


 def setup_uk_locale(dry_run: bool = False) -> None:
     banner("Locale: en_GB + GB keyboard")
-    if dry_run:
-        print("DRY-RUN: dnf install -y langpacks-en_GB")
-        print("DRY-RUN: localectl set-locale LANG=en_GB.UTF-8")
-        print("DRY-RUN: localectl set-x11-keymap gb")
-        return
-    subprocess.run(["dnf", "install", "-y", "langpacks-en_GB"], check=False)
-    subprocess.run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False)
-    subprocess.run(["localectl", "set-x11-keymap", "gb"], check=False)
+    run(["dnf", "install", "-y", "langpacks-en_GB"], check=False, dry_run=dry_run)
+    run(["localectl", "set-locale", "LANG=en_GB.UTF-8"], check=False, dry_run=dry_run)
+    run(["localectl", "set-x11-keymap", "gb"], check=False, dry_run=dry_run)


 def mount_data_partition(user: str, dry_run: bool = False) -> None:
     banner("Storage: ensure DATA partition mounted at /data")
     fstab_line = "LABEL=DATA  /data  ext4  defaults,noatime  0  2\n"

+    run(["mkdir", "-p", "/data"], check=False, dry_run=dry_run)
+
     if dry_run:
-        print("DRY-RUN: mkdir -p /data")
-        print("DRY-RUN: append fstab line if missing: " + fstab_line.strip())
-        print("DRY-RUN: mount -a")
-        print(f"DRY-RUN: chown {user}:{user} /data")
-        return
-
-    os.makedirs("/data", exist_ok=True)
-    try:
-        with open("/etc/fstab", "r", encoding="utf-8") as f:
-            txt = f.read()
-    except FileNotFoundError:
-        txt = ""
-
-    if "/data" not in txt:
-        with open("/etc/fstab", "a", encoding="utf-8") as f:
-            f.write(fstab_line)
-
-    subprocess.run(["mount", "-a"], check=False)
-    subprocess.run(["chown", f"{user}:{user}", "/data"], check=False)
-
-
-def install_starship_fallback(dry_run: bool = False) -> None:
-    """If the starship RPM doesn't exist, install via upstream script to /usr/local/bin."""
-    banner("Starship: fallback install (upstream)")
-    cmd = ["bash", "-lc", "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"]
+        print("DRY-RUN: append fstab line if missing:", fstab_line.strip())
+    else:
+        try:
+            with open("/etc/fstab", "r", encoding="utf-8") as f:
+                txt = f.read()
+        except FileNotFoundError:
+            txt = ""
+        if "/data" not in txt:
+            with open("/etc/fstab", "a", encoding="utf-8") as f:
+                f.write(fstab_line)
+
+    run(["mount", "-a"], check=False, dry_run=dry_run)
+    run(["chown", f"{user}:{user}", "/data"], check=False, dry_run=dry_run)
+
     if dry_run:
-        print("DRY-RUN:", cmd[-1])
-        return
-    subprocess.run(cmd, check=False)
+        print("DRY-RUN: verify mountpoint: findmnt /data")
+    else:
+        subprocess.run(["findmnt", "/data"], check=False)
+
+
+def install_starship(dry_run: bool = False) -> None:
+    if is_installed_rpm("starship"):
+        print("✅ starship already installed (RPM).")
+
+    if not is_installed_rpm("starship") and dnf_pkg_available("starship"):
+        banner("Starship: installing from repos")
+        dnf_install(["starship"], dry_run=dry_run)
+
+    if not is_installed_rpm("starship"):
+        banner("Starship: installing upstream binary to /usr/local/bin")
+        shell = "curl -fsSL https://starship.rs/install.sh | sh -s -- -y -b /usr/local/bin"
+        run(["bash", "-lc", shell], check=False, dry_run=dry_run, shell_hint=shell)
+
+        # Best-effort verification
+        if dry_run:
+            print("DRY-RUN: /usr/local/bin/starship --version")
+        else:
+            subprocess.run(["/usr/local/bin/starship", "--version"], check=False)


 def enable_services(dry_run: bool) -> None:
     banner("Services: firewalld + sshd")
-    cmds = [
-        ["systemctl", "enable", "--now", "firewalld"],
-        ["systemctl", "enable", "--now", "sshd"],
-    ]
-    for cmd in cmds:
-        if dry_run:
-            print("DRY-RUN:", " ".join(shlex.quote(c) for c in cmd))
+    run(["systemctl", "enable", "--now", "firewalld"], check=False, dry_run=dry_run)
+    run(["systemctl", "enable", "--now", "sshd"], check=False, dry_run=dry_run)
+
+    banner("Firewall: allow mosh (best-effort)")
+    run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False, dry_run=dry_run)
+    run(["firewall-cmd", "--reload"], check=False, dry_run=dry_run)
+
+
+def setup_user_qol(target_user: str, dry_run: bool = False) -> None:
+    banner(f"Final QoL: Zsh, Starship, and Power for {target_user}")
+
+    try:
+        user_info = pwd.getpwnam(target_user)
+        home_dir = user_info.pw_dir
+    except KeyError:
+        print(f"❌ User {target_user} not found. Skipping QoL steps.")
+        return
+
+    # 1) Zsh & Starship Setup (Idempotent)
+    zshrc_path = os.path.join(home_dir, ".zshrc")
+    starship_line = 'eval "$(starship init zsh)"'
+
+    if not dry_run:
+        # Ensure .zshrc exists
+        if not os.path.exists(zshrc_path):
+            with open(zshrc_path, "w", encoding="utf-8") as f:
+                f.write("# Zsh Configuration\n")
+            run(["chown", f"{target_user}:{target_user}", zshrc_path], check=False, dry_run=False)
+
+        # Check for existing line before adding
+        try:
+            with open(zshrc_path, "r", encoding="utf-8") as f:
+                content = f.read()
+        except Exception:
+            content = ""
+
+        if starship_line not in content:
+            print(f"✅ Adding Starship init to {zshrc_path}")
+            with open(zshrc_path, "a", encoding="utf-8") as f:
+                f.write(f"\n{starship_line}\n")
+
+        # Change default shell to Zsh for the user (best-effort)
+        if shutil.which("chsh") and os.path.exists("/usr/bin/zsh"):
+            run(["chsh", "-s", "/usr/bin/zsh", target_user], check=False, dry_run=False)
         else:
-            subprocess.run(cmd, check=False)
-
-    banner("Firewall: allow mosh (best-effort)")
-    if dry_run:
-        print("DRY-RUN: firewall-cmd --permanent --add-service=mosh && firewall-cmd --reload")
-        return
-    subprocess.run(["firewall-cmd", "--permanent", "--add-service=mosh"], check=False)
-    subprocess.run(["firewall-cmd", "--reload"], check=False)
+            print("⚠️ chsh or /usr/bin/zsh missing; skipping default shell change.")
+    else:
+        print(f"DRY-RUN: ensure {zshrc_path} exists + append starship init if missing")
+        print(f"DRY-RUN: chsh -s /usr/bin/zsh {target_user}")
+
+    # 2) Disable Screensaver & DPMS (KDE CLI tools) (best-effort)
+    kwrite = shutil.which("kwriteconfig6") or shutil.which("kwriteconfig5")
+    if not kwrite:
+        print("⚠️ kwriteconfig(5/6) not found; skipping KDE config tweaks.")
+    else:
+        kw = os.path.basename(kwrite)
+        kde_cmds = [
+            f"{kw} --file kscreenlockerrc --group Daemon --key Autolock false",
+            f"{kw} --file powerdevilrc --group AC --group SuspendSession --key suspendType 0",
+        ]
+        # xset only works under X11; still try (harmless if it fails).
+        kde_cmds += ["xset s off", "xset -dpms"]
+
+        for cmd in kde_cmds:
+            run(["sudo", "-u", target_user, "sh", "-c", cmd], check=False, dry_run=dry_run)


 def print_summary(categories: List[Category]) -> None:
@@ -261,17 +298,16 @@
 def main() -> int:
     ensure_root()

-    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) — master list installer.")
-    ap.add_argument("--user", default="DrTweak", help="Username to chown /data and set snapshot dir group (default: DrTweak)")
+    ap = argparse.ArgumentParser(description="MASH Bootstrap (Fedora) - master list installer.")
+    ap.add_argument("--user", default="DrTweak", help="Username for QoL steps + /data chown (default: DrTweak)")
     ap.add_argument("--no-upgrade", action="store_true", help="Skip dnf upgrade --refresh")
     ap.add_argument("--no-rpmfusion", action="store_true", help="Do NOT install/enable RPM Fusion repos")
     ap.add_argument("--with-tainted", action="store_true", help="Enable RPM Fusion tainted repos (libdvdcss etc.)")
     ap.add_argument("--with-kodi", action="store_true", help="Install Kodi + key addons")
-    ap.add_argument("--with-snapper", action="store_true", help="Install snapper (already initialized by default; this just keeps it in the list)")
     ap.add_argument("--no-snapper-init", action="store_true", help="Do NOT initialize snapper early")
     ap.add_argument("--no-uk-locale", action="store_true", help="Do NOT force UK locale/keyboard")
     ap.add_argument("--no-data-mount", action="store_true", help="Do NOT setup /data automount")
-    ap.add_argument("--with-starship-fallback", action="store_true", help="If starship RPM missing, install via upstream script")
+    ap.add_argument("--with-starship-fallback", action="store_true", help="Install starship via upstream if RPM missing")
     ap.add_argument("--dry-run", action="store_true", help="Print commands but do not execute")
     args = ap.parse_args()

@@ -283,7 +319,7 @@
     ]))

     categories.append(Category("Dev & build", [
-        "git", "cmake", "ninja-build",  # Fedora package name
+        "git", "cmake", "ninja-build",
         "gcc", "gcc-c++", "ccache",
         "pkgconf", "autoconf", "automake", "libtool",
         "python3-devel", "patchelf",
@@ -296,7 +332,7 @@
         "bat",
         "zsh",
-        "starship",
+        # NOTE: starship handled specially below
     ]))

     categories.append(Category("System tools", [
@@ -350,20 +386,14 @@
             "libdvdcss",
         ]))

-    # Fix typo above safely
-    # (We don't need a separate "with-snapper" category because snapper is initialized before installs.)
-    categories = categories
-
     print_summary(categories)

... (diff truncated) ...
```
