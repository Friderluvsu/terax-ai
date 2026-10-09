# SSH workspace manual QA

Run on two hosts, with the app started from a clean state (`ls ~/.terax/cm` is empty):

- Linux: an OrbStack machine (`orb` alias in `~/.ssh/config`).
- BSD: the iMac (macOS, BSD userland, no GNU tools, `sftp-server` without posix-rename on older systems).

Automated checks first, from the repo root:

```
cd src-tauri && cargo test --lib                       # sftp tests skip without sftp-server
TERAX_SSH_TEST_HOST=<alias> cargo test --lib -- --ignored   # live host test
pnpm test && pnpm build && pnpm size
```

## Checklist (repeat per host)

1. Connect with a password: open the connect dialog, pick the alias, enter the password in the terminal. Explorer loads the home folder.
2. Connect with a key: use a host that has `IdentityFile` set. No prompt; the status indicator turns connected.
3. Host key change: edit `~/.ssh/known_hosts` to corrupt the host entry. Connecting must refuse with a clear message and never auto-accept. Restore the entry afterwards.
4. Network drop: disable Wi-Fi (or suspend the OrbStack machine) for 30 seconds. The indicator shows disconnected, file operations fail without hanging the UI. Restore the network and press Reconnect; the tree and open files work again without restarting the app.
5. Large directory: open a folder with 10,000+ entries (`seq 1 10000 | xargs touch` in a scratch folder). The tree lists it, scrolling stays responsive, deleting the folder is refused above the 10,000 entry limit with the terminal hint.
6. Save with external change: open `a.txt`, edit without saving, change the file from a terminal on the host (`echo x >> a.txt`, wait at least 2 seconds). Save must show the "File changed on disk" toast; Overwrite writes. A change within the same second is not detected (remote mtime is second granular).
7. Save without external change: edit, save, edit again, save again. No false conflict toast. Symlinked file keeps its link after save; file mode is preserved.
8. Search: run a filename and a content search from the explorer. Results open the right remote file.
9. AI read-only: ask the assistant to read a file (works), then to write a file and run a command (both refused with the reason, no approval prompt). Tools are described as unavailable in the SSH workspace.
10. Git: the explorer context menu has no "Open Git History"; the source control panel says git is unavailable; no `git_status` calls while an SSH workspace is active (check the dev console network or logs).
11. Root registration: open a root through a symlink (`ln -s /srv/app ~/appl`). The log shows the server realpath was registered.
12. Quit the app. `ls ~/.terax/cm` is empty and `pgrep -fl "ssh.*ControlPath"` shows no leftover masters.

## Host notes

- iMac: enable Remote Login (System Settings > General > Sharing). Check items 3 and 6 especially; BSD `stat` and `mv` differ from GNU.
- OrbStack: `ssh orb` works out of the box; suspend with `orb stop` for item 4.
