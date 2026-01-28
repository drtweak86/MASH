#-------------------------------------------------------#
# 						        #
# A Guide to using Buildroot to build a Kodi based      #
# appliance						#
#-------------------------------------------------------#

#Setup

Alias

-To set up an alias do the following:
	alias m="make O=../builds/<CURRENT BUILD> BR2_EXTERNAL=../external"

Where the letter or word follwing alias is the alias you wish to create, O being the output folder for buildroot
and BR2_external being the location of the external folder.

Folder Structure

Frankenpi (Top directory)
	-_milestones (Important stages of the build)
		-configs (important configs each stage of a build artifcated one level at a time)
		-frankiesquash (the frankie squashfs experiment...WORKING)
		-frankie (WORKING version of frankie with ext4 wireless and kodi autostart)
	-_backups (backups of work done)
	-backups (more backups...you can never have too many backups!!)
	-buildroot (the buildroot folder obviously where the magic happens)
	-external (where we keep all the important things like the rootfs_overlay)
		-rootfs_overlay (where folders like .kodi live so you can pre-populte it)
		-scripts (where our scripts live like post_build.sh)
		-users (where the user table file lives)
	-builds (the location of all our builds)
		-frankie (the og build where we experiment)
		-frankie2 (the current working iteration of frankie)
		-defcon (DON'T TOUCH OR MESS WITH THIS!!! THIS IS A STANDARD RPI4 64 BUILDROOT DEFAULT CONFIGURATION)
	-old buildroots (where you can find differant iterations of buildroot such as the LTS version)
	-.external (the old external file has gold in it)

# Fixes

If spdlog bails with error:

Make Error in CMakeLists.txt:
  Imported target "fmt::fmt" includes non-existent path

    "/home/drtweak/frankenpi/builds/frankie2/host/aarch64-buildroot-linux-gnu/sysroot/include"

  in its INTERFACE_INCLUDE_DIRECTORIES.  Possible reasons include:

  * The path was deleted, renamed, or moved to another location.

  * An install or uninstall procedure did not complete successfully.

  * The installation package was faulty and references files it does not
  provide.

Fix as follows:

1 - cd ../builds/<CURRENT BUILD>/host/aarch64-buildroot-linux-gnu/sysroot
2 - confirm files exist with: ls -la usr/include
3 - create a symlink: ln -s usr/include include
4 - return to buildroot folder
5 - m spdlog-dirclean (if alias m set up) or make O=../builds/<CURRENT BUILD> BR2_EXTERNAL=../external spdlog-dirclean
6 - m spdlog (if alias m set up) or make O=../builds/<CURRENT BUILD> BR2_EXTERNAL=../external spdlog
7 - If succeeds celebrate the win and continue running the build
