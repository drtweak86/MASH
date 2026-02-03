# History for ninja.py
Chosen master: **ninja-mbr4-final.py** (sha fbb3500eb25c)

## Variants
- ninja-mbr4-final.py (sha fbb3500eb25c)
- ninja-mbr4-updated.py (sha e7028e578c58)
- ninja-mbr4-v2.py (sha 08438020935e)
- ninja-mbr4.py (sha fbb3500eb25c)
- ninja-mbr4(1).py (sha 12bbd775ac92)
- ninja-mbr4(2).py (sha bf0e2a3d315e)
- ninja-mbr4(3).py (sha f5fa281c059c)

## Diffs (against master)

### ninja-mbr4-updated.py → ninja-mbr4-final.py

```diff
--- ninja-mbr4-updated.py
+++ ninja-mbr4-final.py
@@ -470,6 +470,12 @@
             if has_home:
                 mkdirp(root_path / "home")

+# Bind /boot and /boot/efi into the chroot (critical for grub+bls+mkconfig)
+mkdirp(root_path / "boot")
+mkdirp(root_path / "boot" / "efi")
+sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
+sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
+
             # Bind the other btrfs subvols into place (important for a sane chroot)
             if has_var:
                 sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
@@ -511,6 +517,43 @@
             else:
                 print("⚠️ Could not verify password state (passwd -S returned nothing).")

+# ---- GRUB (removable) + absolute config + BLS sync ----
+banner("Installing GRUB (removable) + generating config + BLS sync")
+grub_cmd = "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force"
+config_cmd = "grub2-mkconfig -o /boot/grub2/grub.cfg"
+bls_cmd = "grub2-switch-to-blscfg"
+
+sh(["chroot", str(root_path), "sh", "-c", grub_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", config_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", bls_cmd], check=False)
+
+# ---- SELinux + home-dir safety net ----
+banner("SELinux relabel + home directory safety net")
+relabel_cmd = "touch /.autorelabel"
+home_cmd = "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"
+
+sh(["chroot", str(root_path), "sh", "-c", relabel_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", home_cmd], check=False)
+
+# ---- Verify GRUB + relabel + home ----
+banner("Verifying GRUB + relabel + home")
+bootaa64 = root_path / "boot" / "efi" / "EFI" / "BOOT" / "BOOTAA64.EFI"
+grubcfg = root_path / "boot" / "grub2" / "grub.cfg"
+autorelabel = root_path / ".autorelabel"
+homedir = root_path / "home" / "drtweak"
+
+print(f"  {'✅' if bootaa64.exists() else '❌'} {bootaa64}")
+print(f"  {'✅' if grubcfg.exists() and grubcfg.stat().st_size > 0 else '❌'} {grubcfg}")
+print(f"  {'✅' if autorelabel.exists() else '❌'} {autorelabel}")
+print(f"  {'✅' if homedir.exists() else '❌'} {homedir}")
+
+if grubcfg.exists():
+    # best-effort sanity: mention if it still looks empty
+    head = grubcfg.read_text(errors="ignore")[:4000]
+    if "menuentry" not in head:
+        print("⚠️ /boot/grub2/grub.cfg does not appear to contain 'menuentry' near the top. If GRUB menu is empty, re-check BLS entries on /boot/loader/entries.")
+
+
         # ---- final sanity ----
         banner("Final sanity checks")
         must = ["start4.elf", "fixup4.dat", "RPI_EFI.fd", "EFI/BOOT/BOOTAA64.EFI", "config.txt", "EFI/fedora/grub.cfg"]
@@ -524,6 +567,9 @@

         print("\nfstab written at:")
         print(f"  {target_fstab}")
+
+        banner("Syncing writes to disk")
+        sh(["sync"], check=False)

         banner("DONE")
         print("✅ Flash complete (MBR + 4 partitions).")
```

### ninja-mbr4-v2.py → ninja-mbr4-final.py

```diff
--- ninja-mbr4-v2.py
+++ ninja-mbr4-final.py
@@ -155,48 +155,41 @@
     path.write_text(content, encoding="utf-8")


-def patch_bls_entries(boot_entries_dir: Path, root_uuid: str) -> bool:
-    """
-    Force the exact kernel 'options' flags that were proven to boot successfully:
-      root=UUID=<root_uuid> rootflags=subvol=root rw rhgb quiet
-
-    Returns True if all *.conf files were patched and verified, else False.
-    """
+def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
     if not boot_entries_dir.exists():
-        print(f"⚠️ No BLS entries found at {boot_entries_dir}")
-        return False
-
-    files = sorted(boot_entries_dir.glob("*.conf"))
-    if not files:
-        print(f"⚠️ No BLS entry files in {boot_entries_dir}")
-        return False
-
+        return
+    print(f"🩹 Hard-patching BLS entries for 4TB auto-boot...")
     expected = f"options root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
-    print("🩹 Hard-patching BLS entries for 4TB auto-boot...")
-    ok = True
-    for f in files:
-        lines = f.read_text(encoding="utf-8", errors="ignore").splitlines()
+    bad = []
+    touched = 0
+    for f in boot_entries_dir.glob("*.conf"):
+        lines = f.read_text().splitlines()
         new_lines = []
         for line in lines:
             if line.startswith("options "):
-                new_lines.append(expected)
+                # Force the exact flags that worked manually for you
+                opts = f"root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
+                new_lines.append(f"options {opts}")
             else:
                 new_lines.append(line)
-        f.write_text("\n".join(new_lines) + "\n", encoding="utf-8")
-
-        # Verify immediately
-        after = f.read_text(encoding="utf-8", errors="ignore").splitlines()
-        if expected not in after:
-            ok = False
-            print(f"❌ BLS verify failed: {f}")
-        else:
-            print(f"✅ {f.name}")
-
-    if not ok:
-        print("⚠️ One or more BLS files did not verify. You may get an empty GRUB menu.")
-    return ok
-
-
+        f.write_text("\n".join(new_lines) + "\n")
+        touched += 1
+        # Verify the expected options line is present
+        try:
+            if expected not in f.read_text():
+                bad.append(f.name)
+        except Exception:
+            bad.append(f.name)
+
+    if touched == 0:
+        print("⚠️ No *.conf files found under loader/entries; nothing to patch.")
+    elif bad:
+        print("⚠️ Some BLS entries did not verify after patching:")
+        for name in bad:
+            print(f"   - {name}")
+        print("   (They may be read-only or have unusual formatting.)")
+    else:
+        print("✅ BLS entries patched + verified.")


 def write_grub_stub(efi_fedora_dir: Path, boot_uuid: str):
@@ -463,33 +456,44 @@
         # Patch BLS entries (on /boot)
         patch_bls_entries(DST / "boot" / "loader" / "entries", root_uuid)

-                # ---- dracut + finalize in chroot ----
-        root_path = DST / "root_sub_root"
+        # ---- dracut in chroot ----
         if not args.no_dracut:
-            banner("Bind mounts for chroot")
+            banner("Bind mounts for chroot + fixing /var/tmp + devpts")
+
+            root_path = DST / "root_sub_root"

             # Ensure mountpoints exist in the target rootfs
-            for rel in ["boot", "boot/efi", "data", "dev/pts", "proc", "sys", "run", "tmp", "var/tmp"]:
-                mkdirp(root_path / rel)
+            mkdirp(root_path / "boot" / "efi")
+            mkdirp(root_path / "data")
+            if has_var:
+                mkdirp(root_path / "var")
+            if has_home:
+                mkdirp(root_path / "home")
+
+# Bind /boot and /boot/efi into the chroot (critical for grub+bls+mkconfig)
+mkdirp(root_path / "boot")
+mkdirp(root_path / "boot" / "efi")
+sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
+sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
+
+            # Bind the other btrfs subvols into place (important for a sane chroot)
+            if has_var:
+                sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
+            if has_home:
+                sh(["mount", "--bind", str(DST / "root_sub_home"), str(root_path / "home")])
+
+            # Bind /data for completeness (not strictly required for dracut)
+            sh(["mount", "--bind", str(DST / "data"), str(root_path / "data")])
+
+            # Standard chroot bind mounts
+            for p in ["dev", "proc", "sys", "run", "tmp"]:
+                mkdirp(root_path / p)
+            mkdirp(root_path / "dev" / "pts")
+            mkdirp(root_path / "var" / "tmp")
+
             sh(["chmod", "1777", str(root_path / "tmp")], check=False)
             sh(["chmod", "1777", str(root_path / "var" / "tmp")], check=False)

-            # Bind critical partitions into the chroot (/, /boot, /boot/efi)
-            sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
-            sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
-
-            # Bind the other btrfs subvols into place (important for a sane chroot)
-            if has_var:
-                mkdirp(root_path / "var")
-                sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
-            if has_home:
-                mkdirp(root_path / "home")
-                sh(["mount", "--bind", str(DST / "root_sub_home"), str(root_path / "home")])
-
-            # Bind /data for completeness (not strictly required for dracut)
-            sh(["mount", "--bind", str(DST / "data"), str(root_path / "data")])
-
-            # Standard chroot bind mounts
             sh(["mount", "--bind", "/dev", str(root_path / "dev")])
             sh(["mount", "-t", "devpts", "devpts", str(root_path / "dev" / "pts")], check=False)
             sh(["mount", "--bind", "/proc", str(root_path / "proc")])
@@ -500,56 +504,55 @@
             banner("Running dracut in chroot (regenerate all)")
             sh(["chroot", str(root_path), "dracut", "--regenerate-all", "--force"], check=False)

-            banner("Forcing password reset (user 'fedora')")
-
-            # If the Fedora RAW image doesn't ship with a pre-created user (common),
-            # create one and set the password so you have a guaranteed way in.
-            pw_cmd = r"""if getent passwd fedora >/dev/null 2>&1; then
-  echo 'fedora:fedora123' | chpasswd
-  passwd -S fedora || true
-else
-  echo "⚠️ user 'fedora' not found in target image; creating it..."
-  if getent passwd 1000 >/dev/null 2>&1; then
-    useradd -m -G wheel fedora || true
-  else
-    useradd -m -u 1000 -G wheel fedora || true
-  fi
-  echo 'fedora:fedora123' | chpasswd || true
-  passwd -S fedora || true
-fi
-"""
-            sh(["chroot", str(root_path), "sh", "-c", pw_cmd], check=False)
-
-            banner("Applying Final Boot & Security Fixes")
-
-            # 1) Permanent GRUB Fix (the 'Removable' path)
-            grub_cmds = [
-                "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force",
-                "grub2-mkconfig -o /boot/grub2/grub.cfg",
-                "grub2-switch-to-blscfg",
-            ]
-            for cmd in grub_cmds:
-                sh(["chroot", str(root_path), "sh", "-c", cmd], check=False)
-
-            # 2) Login & Security Fix (SELinux relabel + home dir safety net)
-            sh(["chroot", str(root_path), "sh", "-c", "touch /.autorelabel"], check=False)
-            sh(["chroot", str(root_path), "sh", "-c", "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"], check=False)
-
-            banner("Finalize verification (inside chroot)")
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -f /boot/efi/EFI/BOOT/BOOTAA64.EFI && echo '✅ BOOTAA64.EFI present' || echo '❌ BOOTAA64.EFI missing'"],
-               check=False)
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -f /boot/grub2/grub.cfg && echo '✅ /boot/grub2/grub.cfg present' || echo '❌ /boot/grub2/grub.cfg missing'"],
-               check=False)
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -f /.autorelabel && echo '✅ /.autorelabel present' || echo '❌ /.autorelabel missing'"],
-               check=False)
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -d /home/drtweak && echo '✅ /home/drtweak present' || echo '❌ /home/drtweak missing'"],
-               check=False)
-        else:
-            banner("Skipping dracut/chroot finalization (--no-dracut)")
+            banner("Forcing password reset for user 'fedora'")
+            sh(["chroot", str(root_path), "sh", "-c", "echo 'fedora:fedora123' | chpasswd"], check=False)
+            # Verify the account is not locked (best-effort)
+            status = sh(["chroot", str(root_path), "passwd", "-S", "fedora"], check=False, capture=True)
+            if status:
+                print(f"   passwd -S: {status}")
+                parts = status.split()
+                state = parts[1] if len(parts) > 1 else ""
+                if state not in ("P", "PS"):
+                    print("⚠️ Password state did not look 'set' (state != P/PS). You may need to reset manually.")
+            else:
+                print("⚠️ Could not verify password state (passwd -S returned nothing).")
+
+# ---- GRUB (removable) + absolute config + BLS sync ----
+banner("Installing GRUB (removable) + generating config + BLS sync")
+grub_cmd = "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force"
+config_cmd = "grub2-mkconfig -o /boot/grub2/grub.cfg"
+bls_cmd = "grub2-switch-to-blscfg"
+
+sh(["chroot", str(root_path), "sh", "-c", grub_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", config_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", bls_cmd], check=False)
+
+# ---- SELinux + home-dir safety net ----
+banner("SELinux relabel + home directory safety net")
+relabel_cmd = "touch /.autorelabel"
+home_cmd = "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"
+
+sh(["chroot", str(root_path), "sh", "-c", relabel_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", home_cmd], check=False)
+
+# ---- Verify GRUB + relabel + home ----
+banner("Verifying GRUB + relabel + home")
+bootaa64 = root_path / "boot" / "efi" / "EFI" / "BOOT" / "BOOTAA64.EFI"
+grubcfg = root_path / "boot" / "grub2" / "grub.cfg"
+autorelabel = root_path / ".autorelabel"
+homedir = root_path / "home" / "drtweak"
+
+print(f"  {'✅' if bootaa64.exists() else '❌'} {bootaa64}")
+print(f"  {'✅' if grubcfg.exists() and grubcfg.stat().st_size > 0 else '❌'} {grubcfg}")
+print(f"  {'✅' if autorelabel.exists() else '❌'} {autorelabel}")
+print(f"  {'✅' if homedir.exists() else '❌'} {homedir}")
+
+if grubcfg.exists():
+    # best-effort sanity: mention if it still looks empty
+    head = grubcfg.read_text(errors="ignore")[:4000]
+    if "menuentry" not in head:
+        print("⚠️ /boot/grub2/grub.cfg does not appear to contain 'menuentry' near the top. If GRUB menu is empty, re-check BLS entries on /boot/loader/entries.")
+

         # ---- final sanity ----
         banner("Final sanity checks")
@@ -564,6 +567,9 @@

         print("\nfstab written at:")
         print(f"  {target_fstab}")
+
+        banner("Syncing writes to disk")
+        sh(["sync"], check=False)

         banner("DONE")
         print("✅ Flash complete (MBR + 4 partitions).")
```

### ninja-mbr4.py → ninja-mbr4-final.py

```diff

```

### ninja-mbr4(1).py → ninja-mbr4-final.py

```diff
--- ninja-mbr4(1).py
+++ ninja-mbr4-final.py
@@ -156,49 +156,40 @@


 def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
-    """
-    Patch BLS entries under /boot/loader/entries/*.conf:
-      - set root=UUID=<new>
-      - ensure rootflags=subvol=root
-    """
     if not boot_entries_dir.exists():
-        print(f"⚠️  No BLS entries dir found: {boot_entries_dir} (skipping BLS patch)")
         return
-
-    files = sorted(boot_entries_dir.glob("*.conf"))
-    if not files:
-        print(f"⚠️  No BLS entry files in {boot_entries_dir} (skipping BLS patch)")
-        return
-
-    print(f"🩹 Patching BLS entries in {boot_entries_dir} ...")
-    for f in files:
-        txt = f.read_text(encoding="utf-8", errors="ignore").splitlines(True)
-
-        out = []
-        for line in txt:
+    print(f"🩹 Hard-patching BLS entries for 4TB auto-boot...")
+    expected = f"options root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
+    bad = []
+    touched = 0
+    for f in boot_entries_dir.glob("*.conf"):
+        lines = f.read_text().splitlines()
+        new_lines = []
+        for line in lines:
             if line.startswith("options "):
-                opts = line[len("options "):].strip()
-
-                # replace root=...
-                if re.search(r"\broot=UUID=[0-9a-fA-F-]+\b", opts):
-                    opts = re.sub(r"\broot=UUID=[0-9a-fA-F-]+\b", f"root=UUID={root_uuid}", opts)
-                elif re.search(r"\broot=[^\s]+\b", opts):
-                    opts = re.sub(r"\broot=[^\s]+\b", f"root=UUID={root_uuid}", opts)
-                else:
-                    opts = f"root=UUID={root_uuid} " + opts
-
-                # ensure rootflags=subvol=root
-                if re.search(r"\brootflags=", opts):
-                    opts = re.sub(r"\brootflags=[^\s]+\b", "rootflags=subvol=root", opts)
-                else:
-                    opts = opts + " rootflags=subvol=root"
-
-                out.append("options " + opts.strip() + "\n")
+                # Force the exact flags that worked manually for you
+                opts = f"root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
+                new_lines.append(f"options {opts}")
             else:
-                out.append(line)
-
-        f.write_text("".join(out), encoding="utf-8")
-    print("✅ BLS patched.")
+                new_lines.append(line)
+        f.write_text("\n".join(new_lines) + "\n")
+        touched += 1
+        # Verify the expected options line is present
+        try:
+            if expected not in f.read_text():
+                bad.append(f.name)
+        except Exception:
+            bad.append(f.name)
+
+    if touched == 0:
+        print("⚠️ No *.conf files found under loader/entries; nothing to patch.")
+    elif bad:
+        print("⚠️ Some BLS entries did not verify after patching:")
+        for name in bad:
+            print(f"   - {name}")
+        print("   (They may be read-only or have unusual formatting.)")
+    else:
+        print("✅ BLS entries patched + verified.")


 def write_grub_stub(efi_fedora_dir: Path, boot_uuid: str):
@@ -479,6 +470,12 @@
             if has_home:
                 mkdirp(root_path / "home")

+# Bind /boot and /boot/efi into the chroot (critical for grub+bls+mkconfig)
+mkdirp(root_path / "boot")
+mkdirp(root_path / "boot" / "efi")
+sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
+sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
+
             # Bind the other btrfs subvols into place (important for a sane chroot)
             if has_var:
                 sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
@@ -506,6 +503,56 @@

             banner("Running dracut in chroot (regenerate all)")
             sh(["chroot", str(root_path), "dracut", "--regenerate-all", "--force"], check=False)
+
+            banner("Forcing password reset for user 'fedora'")
+            sh(["chroot", str(root_path), "sh", "-c", "echo 'fedora:fedora123' | chpasswd"], check=False)
+            # Verify the account is not locked (best-effort)
+            status = sh(["chroot", str(root_path), "passwd", "-S", "fedora"], check=False, capture=True)
+            if status:
+                print(f"   passwd -S: {status}")
+                parts = status.split()
+                state = parts[1] if len(parts) > 1 else ""
+                if state not in ("P", "PS"):
+                    print("⚠️ Password state did not look 'set' (state != P/PS). You may need to reset manually.")
+            else:
+                print("⚠️ Could not verify password state (passwd -S returned nothing).")
+
+# ---- GRUB (removable) + absolute config + BLS sync ----
+banner("Installing GRUB (removable) + generating config + BLS sync")
+grub_cmd = "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force"
+config_cmd = "grub2-mkconfig -o /boot/grub2/grub.cfg"
+bls_cmd = "grub2-switch-to-blscfg"
+
+sh(["chroot", str(root_path), "sh", "-c", grub_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", config_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", bls_cmd], check=False)
+
+# ---- SELinux + home-dir safety net ----
+banner("SELinux relabel + home directory safety net")
+relabel_cmd = "touch /.autorelabel"
+home_cmd = "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"
+
+sh(["chroot", str(root_path), "sh", "-c", relabel_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", home_cmd], check=False)
+
+# ---- Verify GRUB + relabel + home ----
+banner("Verifying GRUB + relabel + home")
+bootaa64 = root_path / "boot" / "efi" / "EFI" / "BOOT" / "BOOTAA64.EFI"
+grubcfg = root_path / "boot" / "grub2" / "grub.cfg"
+autorelabel = root_path / ".autorelabel"
+homedir = root_path / "home" / "drtweak"
+
+print(f"  {'✅' if bootaa64.exists() else '❌'} {bootaa64}")
+print(f"  {'✅' if grubcfg.exists() and grubcfg.stat().st_size > 0 else '❌'} {grubcfg}")
+print(f"  {'✅' if autorelabel.exists() else '❌'} {autorelabel}")
+print(f"  {'✅' if homedir.exists() else '❌'} {homedir}")
+
+if grubcfg.exists():
+    # best-effort sanity: mention if it still looks empty
+    head = grubcfg.read_text(errors="ignore")[:4000]
+    if "menuentry" not in head:
+        print("⚠️ /boot/grub2/grub.cfg does not appear to contain 'menuentry' near the top. If GRUB menu is empty, re-check BLS entries on /boot/loader/entries.")
+

         # ---- final sanity ----
         banner("Final sanity checks")
@@ -520,6 +567,9 @@

         print("\nfstab written at:")
         print(f"  {target_fstab}")
+
+        banner("Syncing writes to disk")
+        sh(["sync"], check=False)

         banner("DONE")
         print("✅ Flash complete (MBR + 4 partitions).")
```

### ninja-mbr4(2).py → ninja-mbr4-final.py

```diff
--- ninja-mbr4(2).py
+++ ninja-mbr4-final.py
@@ -156,27 +156,40 @@


 def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
-    """
-    Force-patches all kernel boot files to ensure they point to the 4TB drive.
-    """
     if not boot_entries_dir.exists():
-        print(f"⚠️ No BLS entries found at {boot_entries_dir}")
         return
-
     print(f"🩹 Hard-patching BLS entries for 4TB auto-boot...")
+    expected = f"options root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
+    bad = []
+    touched = 0
     for f in boot_entries_dir.glob("*.conf"):
         lines = f.read_text().splitlines()
         new_lines = []
         for line in lines:
             if line.startswith("options "):
-                # This is the line we manual-typed earlier.
-                # We are forcing it to include the correct UUID and Btrfs subvol.
+                # Force the exact flags that worked manually for you
                 opts = f"root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
                 new_lines.append(f"options {opts}")
             else:
                 new_lines.append(line)
         f.write_text("\n".join(new_lines) + "\n")
-    print("✅ BLS files updated. The menu should no longer be empty.")
+        touched += 1
+        # Verify the expected options line is present
+        try:
+            if expected not in f.read_text():
+                bad.append(f.name)
+        except Exception:
+            bad.append(f.name)
+
+    if touched == 0:
+        print("⚠️ No *.conf files found under loader/entries; nothing to patch.")
+    elif bad:
+        print("⚠️ Some BLS entries did not verify after patching:")
+        for name in bad:
+            print(f"   - {name}")
+        print("   (They may be read-only or have unusual formatting.)")
+    else:
+        print("✅ BLS entries patched + verified.")


 def write_grub_stub(efi_fedora_dir: Path, boot_uuid: str):
@@ -449,14 +462,6 @@

             root_path = DST / "root_sub_root"

-
-
-
-
-            # Force-reset the 'fedora' user password to 'fedora123' (change this as you like!)
-            banner("Resetting user password")
-            sh(["chroot", str(root_path), "sh", "-c", "echo 'fedora:fedora123' | chpasswd"], check=False)
-
             # Ensure mountpoints exist in the target rootfs
             mkdirp(root_path / "boot" / "efi")
             mkdirp(root_path / "data")
@@ -465,6 +470,12 @@
             if has_home:
                 mkdirp(root_path / "home")

+# Bind /boot and /boot/efi into the chroot (critical for grub+bls+mkconfig)
+mkdirp(root_path / "boot")
+mkdirp(root_path / "boot" / "efi")
+sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
+sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
+
             # Bind the other btrfs subvols into place (important for a sane chroot)
             if has_var:
                 sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
@@ -492,6 +503,56 @@

             banner("Running dracut in chroot (regenerate all)")
             sh(["chroot", str(root_path), "dracut", "--regenerate-all", "--force"], check=False)
+
+            banner("Forcing password reset for user 'fedora'")
+            sh(["chroot", str(root_path), "sh", "-c", "echo 'fedora:fedora123' | chpasswd"], check=False)
+            # Verify the account is not locked (best-effort)
+            status = sh(["chroot", str(root_path), "passwd", "-S", "fedora"], check=False, capture=True)
+            if status:
+                print(f"   passwd -S: {status}")
+                parts = status.split()
+                state = parts[1] if len(parts) > 1 else ""
+                if state not in ("P", "PS"):
+                    print("⚠️ Password state did not look 'set' (state != P/PS). You may need to reset manually.")
+            else:
+                print("⚠️ Could not verify password state (passwd -S returned nothing).")
+
+# ---- GRUB (removable) + absolute config + BLS sync ----
+banner("Installing GRUB (removable) + generating config + BLS sync")
+grub_cmd = "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force"
+config_cmd = "grub2-mkconfig -o /boot/grub2/grub.cfg"
+bls_cmd = "grub2-switch-to-blscfg"
+
+sh(["chroot", str(root_path), "sh", "-c", grub_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", config_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", bls_cmd], check=False)
+
+# ---- SELinux + home-dir safety net ----
+banner("SELinux relabel + home directory safety net")
+relabel_cmd = "touch /.autorelabel"
+home_cmd = "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"
+
+sh(["chroot", str(root_path), "sh", "-c", relabel_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", home_cmd], check=False)
+
+# ---- Verify GRUB + relabel + home ----
+banner("Verifying GRUB + relabel + home")
+bootaa64 = root_path / "boot" / "efi" / "EFI" / "BOOT" / "BOOTAA64.EFI"
+grubcfg = root_path / "boot" / "grub2" / "grub.cfg"
+autorelabel = root_path / ".autorelabel"
+homedir = root_path / "home" / "drtweak"
+
+print(f"  {'✅' if bootaa64.exists() else '❌'} {bootaa64}")
+print(f"  {'✅' if grubcfg.exists() and grubcfg.stat().st_size > 0 else '❌'} {grubcfg}")
+print(f"  {'✅' if autorelabel.exists() else '❌'} {autorelabel}")
+print(f"  {'✅' if homedir.exists() else '❌'} {homedir}")
+
+if grubcfg.exists():
+    # best-effort sanity: mention if it still looks empty
+    head = grubcfg.read_text(errors="ignore")[:4000]
+    if "menuentry" not in head:
+        print("⚠️ /boot/grub2/grub.cfg does not appear to contain 'menuentry' near the top. If GRUB menu is empty, re-check BLS entries on /boot/loader/entries.")
+

         # ---- final sanity ----
         banner("Final sanity checks")
@@ -506,6 +567,9 @@

         print("\nfstab written at:")
         print(f"  {target_fstab}")
+
+        banner("Syncing writes to disk")
+        sh(["sync"], check=False)

         banner("DONE")
         print("✅ Flash complete (MBR + 4 partitions).")
```

### ninja-mbr4(3).py → ninja-mbr4-final.py

```diff
--- ninja-mbr4(3).py
+++ ninja-mbr4-final.py
@@ -155,48 +155,41 @@
     path.write_text(content, encoding="utf-8")


-def patch_bls_entries(boot_entries_dir: Path, root_uuid: str) -> bool:
-    """
-    Force the exact kernel 'options' flags that were proven to boot successfully:
-      root=UUID=<root_uuid> rootflags=subvol=root rw rhgb quiet
-
-    Returns True if all *.conf files were patched and verified, else False.
-    """
+def patch_bls_entries(boot_entries_dir: Path, root_uuid: str):
     if not boot_entries_dir.exists():
-        print(f"⚠️ No BLS entries found at {boot_entries_dir}")
-        return False
-
-    files = sorted(boot_entries_dir.glob("*.conf"))
-    if not files:
-        print(f"⚠️ No BLS entry files in {boot_entries_dir}")
-        return False
-
+        return
+    print(f"🩹 Hard-patching BLS entries for 4TB auto-boot...")
     expected = f"options root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
-    print("🩹 Hard-patching BLS entries for 4TB auto-boot...")
-    ok = True
-    for f in files:
-        lines = f.read_text(encoding="utf-8", errors="ignore").splitlines()
+    bad = []
+    touched = 0
+    for f in boot_entries_dir.glob("*.conf"):
+        lines = f.read_text().splitlines()
         new_lines = []
         for line in lines:
             if line.startswith("options "):
-                new_lines.append(expected)
+                # Force the exact flags that worked manually for you
+                opts = f"root=UUID={root_uuid} rootflags=subvol=root rw rhgb quiet"
+                new_lines.append(f"options {opts}")
             else:
                 new_lines.append(line)
-        f.write_text("\n".join(new_lines) + "\n", encoding="utf-8")
-
-        # Verify immediately
-        after = f.read_text(encoding="utf-8", errors="ignore").splitlines()
-        if expected not in after:
-            ok = False
-            print(f"❌ BLS verify failed: {f}")
-        else:
-            print(f"✅ {f.name}")
-
-    if not ok:
-        print("⚠️ One or more BLS files did not verify. You may get an empty GRUB menu.")
-    return ok
-
-
+        f.write_text("\n".join(new_lines) + "\n")
+        touched += 1
+        # Verify the expected options line is present
+        try:
+            if expected not in f.read_text():
+                bad.append(f.name)
+        except Exception:
+            bad.append(f.name)
+
+    if touched == 0:
+        print("⚠️ No *.conf files found under loader/entries; nothing to patch.")
+    elif bad:
+        print("⚠️ Some BLS entries did not verify after patching:")
+        for name in bad:
+            print(f"   - {name}")
+        print("   (They may be read-only or have unusual formatting.)")
+    else:
+        print("✅ BLS entries patched + verified.")


 def write_grub_stub(efi_fedora_dir: Path, boot_uuid: str):
@@ -463,33 +456,44 @@
         # Patch BLS entries (on /boot)
         patch_bls_entries(DST / "boot" / "loader" / "entries", root_uuid)

-                # ---- dracut + finalize in chroot ----
-        root_path = DST / "root_sub_root"
+        # ---- dracut in chroot ----
         if not args.no_dracut:
-            banner("Bind mounts for chroot")
+            banner("Bind mounts for chroot + fixing /var/tmp + devpts")
+
+            root_path = DST / "root_sub_root"

             # Ensure mountpoints exist in the target rootfs
-            for rel in ["boot", "boot/efi", "data", "dev/pts", "proc", "sys", "run", "tmp", "var/tmp"]:
-                mkdirp(root_path / rel)
+            mkdirp(root_path / "boot" / "efi")
+            mkdirp(root_path / "data")
+            if has_var:
+                mkdirp(root_path / "var")
+            if has_home:
+                mkdirp(root_path / "home")
+
+# Bind /boot and /boot/efi into the chroot (critical for grub+bls+mkconfig)
+mkdirp(root_path / "boot")
+mkdirp(root_path / "boot" / "efi")
+sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
+sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
+
+            # Bind the other btrfs subvols into place (important for a sane chroot)
+            if has_var:
+                sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
+            if has_home:
+                sh(["mount", "--bind", str(DST / "root_sub_home"), str(root_path / "home")])
+
+            # Bind /data for completeness (not strictly required for dracut)
+            sh(["mount", "--bind", str(DST / "data"), str(root_path / "data")])
+
+            # Standard chroot bind mounts
+            for p in ["dev", "proc", "sys", "run", "tmp"]:
+                mkdirp(root_path / p)
+            mkdirp(root_path / "dev" / "pts")
+            mkdirp(root_path / "var" / "tmp")
+
             sh(["chmod", "1777", str(root_path / "tmp")], check=False)
             sh(["chmod", "1777", str(root_path / "var" / "tmp")], check=False)

-            # Bind critical partitions into the chroot (/, /boot, /boot/efi)
-            sh(["mount", "--bind", str(DST / "boot"), str(root_path / "boot")])
-            sh(["mount", "--bind", str(DST / "efi"), str(root_path / "boot" / "efi")])
-
-            # Bind the other btrfs subvols into place (important for a sane chroot)
-            if has_var:
-                mkdirp(root_path / "var")
-                sh(["mount", "--bind", str(DST / "root_sub_var"), str(root_path / "var")])
-            if has_home:
-                mkdirp(root_path / "home")
-                sh(["mount", "--bind", str(DST / "root_sub_home"), str(root_path / "home")])
-
-            # Bind /data for completeness (not strictly required for dracut)
-            sh(["mount", "--bind", str(DST / "data"), str(root_path / "data")])
-
-            # Standard chroot bind mounts
             sh(["mount", "--bind", "/dev", str(root_path / "dev")])
             sh(["mount", "-t", "devpts", "devpts", str(root_path / "dev" / "pts")], check=False)
             sh(["mount", "--bind", "/proc", str(root_path / "proc")])
@@ -502,38 +506,53 @@

             banner("Forcing password reset for user 'fedora'")
             sh(["chroot", str(root_path), "sh", "-c", "echo 'fedora:fedora123' | chpasswd"], check=False)
-            sh(["chroot", str(root_path), "passwd", "-S", "fedora"], check=False)
-
-            banner("Applying Final Boot & Security Fixes")
-
-            # 1) Permanent GRUB Fix (the 'Removable' path)
-            grub_cmds = [
-                "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force",
-                "grub2-mkconfig -o /boot/grub2/grub.cfg",
-                "grub2-switch-to-blscfg",
-            ]
-            for cmd in grub_cmds:
-                sh(["chroot", str(root_path), "sh", "-c", cmd], check=False)
-
-            # 2) Login & Security Fix (SELinux relabel + home dir safety net)
-            sh(["chroot", str(root_path), "sh", "-c", "touch /.autorelabel"], check=False)
-            sh(["chroot", str(root_path), "sh", "-c", "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"], check=False)
-
-            banner("Finalize verification (inside chroot)")
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -f /boot/efi/EFI/BOOT/BOOTAA64.EFI && echo '✅ BOOTAA64.EFI present' || echo '❌ BOOTAA64.EFI missing'"],
-               check=False)
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -f /boot/grub2/grub.cfg && echo '✅ /boot/grub2/grub.cfg present' || echo '❌ /boot/grub2/grub.cfg missing'"],
-               check=False)
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -f /.autorelabel && echo '✅ /.autorelabel present' || echo '❌ /.autorelabel missing'"],
-               check=False)
-            sh(["chroot", str(root_path), "sh", "-c",
-                "test -d /home/drtweak && echo '✅ /home/drtweak present' || echo '❌ /home/drtweak missing'"],
-               check=False)
-        else:
-            banner("Skipping dracut/chroot finalization (--no-dracut)")
+            # Verify the account is not locked (best-effort)
+            status = sh(["chroot", str(root_path), "passwd", "-S", "fedora"], check=False, capture=True)
+            if status:
+                print(f"   passwd -S: {status}")
+                parts = status.split()
+                state = parts[1] if len(parts) > 1 else ""
+                if state not in ("P", "PS"):
+                    print("⚠️ Password state did not look 'set' (state != P/PS). You may need to reset manually.")
+            else:
+                print("⚠️ Could not verify password state (passwd -S returned nothing).")
+
+# ---- GRUB (removable) + absolute config + BLS sync ----
+banner("Installing GRUB (removable) + generating config + BLS sync")
+grub_cmd = "grub2-install --target=arm64-efi --efi-directory=/boot/efi --removable --force"
+config_cmd = "grub2-mkconfig -o /boot/grub2/grub.cfg"
+bls_cmd = "grub2-switch-to-blscfg"
+
+sh(["chroot", str(root_path), "sh", "-c", grub_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", config_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", bls_cmd], check=False)
+
+# ---- SELinux + home-dir safety net ----
+banner("SELinux relabel + home directory safety net")
+relabel_cmd = "touch /.autorelabel"
+home_cmd = "mkdir -p /home/drtweak && chown 1000:1000 /home/drtweak"
+
+sh(["chroot", str(root_path), "sh", "-c", relabel_cmd], check=False)
+sh(["chroot", str(root_path), "sh", "-c", home_cmd], check=False)
+
+# ---- Verify GRUB + relabel + home ----
+banner("Verifying GRUB + relabel + home")
+bootaa64 = root_path / "boot" / "efi" / "EFI" / "BOOT" / "BOOTAA64.EFI"
+grubcfg = root_path / "boot" / "grub2" / "grub.cfg"
+autorelabel = root_path / ".autorelabel"
+homedir = root_path / "home" / "drtweak"
+
+print(f"  {'✅' if bootaa64.exists() else '❌'} {bootaa64}")
+print(f"  {'✅' if grubcfg.exists() and grubcfg.stat().st_size > 0 else '❌'} {grubcfg}")
+print(f"  {'✅' if autorelabel.exists() else '❌'} {autorelabel}")
+print(f"  {'✅' if homedir.exists() else '❌'} {homedir}")
+
+if grubcfg.exists():
+    # best-effort sanity: mention if it still looks empty
+    head = grubcfg.read_text(errors="ignore")[:4000]
+    if "menuentry" not in head:
+        print("⚠️ /boot/grub2/grub.cfg does not appear to contain 'menuentry' near the top. If GRUB menu is empty, re-check BLS entries on /boot/loader/entries.")
+

         # ---- final sanity ----
         banner("Final sanity checks")
@@ -548,6 +567,9 @@

         print("\nfstab written at:")
         print(f"  {target_fstab}")
+
+        banner("Syncing writes to disk")
+        sh(["sync"], check=False)

         banner("DONE")
         print("✅ Flash complete (MBR + 4 partitions).")
```
