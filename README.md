[More info/Buy](https://3gg.se/products/rkbx_link) | [Buy a License](https://store.3gg.se/)

# rkbx_link for Rekordbox
Export rock-solid timing information to sync live lights and music to your DJ sets in Rekordbox! With support for Ableton Link, OSC, setlist generation and more. rkbx_link provides highly accurate low-latency data by reading transport position and beatgrid directly from memory.

With the download of this software you will receive an evaluation license with offsets for Rekordbox 7.1.3. To get support for the latest versions of Rekordbox, [buy a license](https://3gg.se/products/rkbx_link) and get automatic updates! Or if you're using it commercially making loads of dosh, consider extra support on my [ko-fi](https://ko-fi.com/grufkork).

## Usage & Setup
Download the latest version from [the releases](https://github.com/grufkork/rkbx_link/releases/latest). Unzip and edit the `config` file:
- Set the Rekordbox version (`keeper.rekordbox_version`) you are using
- Set the correct numbers of decks (`keeper.decks`) (2 or 4)
- Enable the output modules you want to use, such as `link.enabled` or `osc.enabled`.
Then run `rkbx_link.exe` to start the program. It will automatically connect to Rekordbox and restart if it fails. During startup all available Rekordbox versions are printed.

Check the end of this document for troubleshooting tips.

Eventually you can also tune:
- `keeper.delay_compensation` to compensate for latency in your audio interface, lights or network. You can use both positive and negative values.

## Supported versions (with license)

| Rekordbox Version  |
| ----- |
| `7.1.2`, `7.1.3` |
| `6.8.5` |

![logo](https://3gg.se/products/rkbx_link/logo.png "Logo")

## What does it do?
When run on the same computer as an instance of Rekordbox, the program reads the current timing data and sends this over your protocol of choice. By default it outputs a 4-beat aligned Ableton Link session, but it can also transmit equivalent data over OSC.

The program does not interact with the audio stream in any way, but reads values through memory. It is therefore extremely accurate (provided your beatgrids are correct!)

## Why?
Rekordbox's Ableton Link integration only allows for receiving a signal, not extracting it. 

# Configuration and output modules
Listed below are all available modules and how to configure them. You enable and disable them in the `config` file.

- `app.license <string>`
Enter your license key here to get support for the latest Rekordbox versions. Otherwise leave it empty.

- `app.auto_update <true/false>`
Enables checking for updates on startup if you have a valid [license](https://3gg.se/products/rkbx_link). 

## Keeper (settings for beat tracking)
- `keeper.rekordbox_version <string>`
Enter the version of Rekordbox to target (eg. 6.8.5 or 7.1.3). You can see available versions on this page or when starting the program. 

- `keeper.update_rate <int>`
Number of updates per second to send. Default is 120, which results in about 60 updates per second due to Windows' sleep granularity. You can set this lower if you want to save CPU usage, but it will result in less accurate timing.

- `keeper.slow_update_every_nth <int>`
How often to read additional data from Rekordbox. Saves a bit of CPU usage if increased, but will not really affect worst-case performance. Default is `10`, meaning every 10th update will read the current track name and artist.

- `keeper.delay_compensation <float>`
Time in milliseconds to shift the output. Used to compensate for latency in audio, network, lights etc. Can be both negative and positive to either delay the signal or compensate for latency down the chain.

- `keeper.bar_jitter_tolerance <int>`
Due to some technicalities with how values are read, dead reckoning is used to smooth out 4-beat sized jitter. After this number of updates, the jitter is no longer considered jitter and the new position is considered the correct value. Default is 10.

- `keeper.keep_warm <true/false>`
Enabling this means all decks are tracked even when not active. Enabling this increases CPU usage a bit, but means that when you switch decks the new one will already be warmed up and ready to go. Default is `true`.

- `keeper.decks <int>`
Number of decks to track, 1 to 4. This decides how many decks are read from Rekordbox's memory. If you choose more decks than are active in Rekordbox, the program will try to read decks where the are not any and fail to connect.

## Open Sound Control (OSC)
Outputs transport and more data over OSC. Check below for all addresses.
- `osc.enabled <true/false>`
Whether to enable OSC output.

- `osc.source <IP address>`
Local address to bind to. Default is 127.0.0.1:4450

- `osc.destination <IP address>`
Address to send OSC messages to. Default is 127.0.0.1:4460

## Ableton Link
- `link.enabled <true/false>`
Whether to enable Ableton Link output.

- `link.cumulative_error_tolerance <float>`
Cumulative error in beats allowed before a resync is triggered. Default is 0.05. Lower or set to zero if you really want it to track when you scratch, otherwise leave as is to save a bit of CPU and network (and to be nicer to other peers).

## Track to file
- `file.enabled <true/false>`
Whether to write the current master track to a file. Title, artist and album are written to separate lines.

- `file.filename <string>`
Filename to write the current track to. Default is `current_track.txt` in the same directory as the executable.

## Setlist to file
This module logs the current master track to a setlist file together with when it was played relative to setlist start. The first line in the file contains the setlist start time in Unix time. On startup, if there already is a setlist file, it will continue appending to it with timestamps relative to the creation of the setlist.

- `setlist.enabled <true/false>`
Whether to enable setlist output.

- `setlist.separator <string>`
Separator to use between title and artist in the setlist file. Default is `-`.

- `setlist.filename <string>`
Where to write the setlist file. Default is `setlist.txt` in the same directory as the executable.

## OSC Addresses
 - `/bpm/current` (float) Current BPM of the master deck
 - `/bpm/original` (float) Original (non-pitched) BPM of the master deck
 - `/beat` (float) Total beat / number of beats since beat 1.1
 - `/beat/[1|2|4]` (float) Normalised values 0-1 looping with 1, 2 or 4 beat intervals.
 - `/time` (float) Current track position in seconds
 - `/playback_speed` (float) Current playback speed/pitch, 1.0 for normal speed, 2.0 for double speed etc.
 - `/track/[1|2|3|4|master]/[title|artist|album]` (string) Title/artist/album of the current track on deck 1, 2, 3 or 4, or the master deck.

# Troubleshooting
Try the following if you run into issues. If you even after going through all these still are having problems, please [open an issue](https://github.com/grufkork/rkbx_link/issues/new) on GitHub.

### The program fails to connect to Rekordbox
- Make sure you have selected the correct Rekordbox version in the config file.
- Check that have the correct number of decks set in the config file. Selecting 4 decks when you only have 2 will prevent the program from connecting.
- Esnure Rekordbox is running and has a track loaded in the deck you are trying to read.
- Try updating the program or the offsets.

### Some decks are not working
Make sure you have the correct number of decks set in the config file.

### The program starts and immediately disappears
A catastrophic failure has occurred. Open a command prompt in the directory where rkbx_link.exe is located and run `rkbx_link.exe` from there. You can now see the error in the console. You will probably want to enable debug in the config, copy the output and open an issue on GitHub.
