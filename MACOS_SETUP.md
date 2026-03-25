# macOS Setup for rkbx_link

## Re-signing Rekordbox

macOS prevents reading memory from other processes, even with `sudo`. This is due to:
1. System Integrity Protection (SIP)
2. Rekordbox being notarized by Apple
3. Rekordbox lacking the `get-task-allow` entitlement

Instead of disabling SIP, we re-sign Rekordbox ourselves to add the `get-task-allow` entitlement.
Use the `resign_rekordbox.sh` script to remove the signature, update the entitlements, and re-sign.

What This Does
- ✅ Removes Apple's notarization (allows library injection if needed)
- ✅ Adds `get-task-allow` entitlement (allows memory reading)
- ✅ Keeps all Rekordbox's original entitlements (audio, camera, etc.)
- ⚠️  Rekordbox will show security warning on first launch (right-click → Open to bypass)

After re-signing:
- `task_for_pid()` succeeds
- Memory reading works with `sudo ./rkbx_link`
- All SIP protections remain active

This is a one-time command and will persist after reboots.

## Scanning Offsets

- Finding BPM is pretty easy, and then the other BPMs are all close by
  - They share a similar offset structure as the original Windows offsets from 7.2.2
- Finding position
  - Use the memory_browser bin to find the sample position based on the BPM address
    - Move the scrubber around +, -, and 0 a few times and it will be obvious
    - Sample rate is 44100 so moving half a second will be about 22k samples
- Finding song title is easy, search string "Title: x" and there's only one result, heap allocated
  - Load songs onto each deck
  - Find the addresses for all 4 songs
  - Generate a pointermap (original)
  - Change all 4 songs in Rekordbox
  - Search for the new addresses
  - Generate a second pointermap (updated)
  - For each new address (eg track 1)
    - Run a pointer scan on it, use saved pointermap (updated), compare to (original) with original address
    - I ran it with static-only OFF but that may not be necessary
  - Find the ones that are most similar after all 4 scans
- Finding ANLZ path
  - Find the ANLZ path by analyzing a song and then using the anlz_paths tool to find the most recently updated file
  - String search for the .DAT file, grab the address thats on the heap, there should only be 1
  - Generate pointermap 1
  - Unload and reload the same song from the same deck
  - Generate pointermap 2
  - Pointer scan, static, 3 deep
  - Compare pointermap 2 against pointermap 1

## Test Programs

### resign_rekordbox.sh

Removes Apple notarized signature and re-signs with our own.
Used to add the allow-get-task entitlement so we can read Rekordbox memory.
