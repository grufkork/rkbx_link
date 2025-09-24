[More info/Buy](https://3gg.se/products/rkbx_link) | [Buy a License](https://store.3gg.se/)

# rkbx_link for Rekordbox 
Export live and rock-solid timing, phrase and track info to sync live lights and music to your DJ sets in Rekordbox! With support for Ableton Link, OSC, sACN, setlist generation and more. rkbx_link provides highly accurate low-latency data by reading transport position and beatgrid directly from memory. Essentially it's Pro DJ Link, but for Rekordbox!

With the download of this software you will receive an evaluation license with offsets for Rekordbox 7.2.2. To get support for the latest versions of Rekordbox, [buy a license](https://3gg.se/products/rkbx_link) and get automatic updates! Or if you're using it commercially making loads of dosh, consider extra support on my [ko-fi](https://ko-fi.com/grufkork).

<img src="./logo_tiny.png" width="150">

<details>
  <summary>Contents</summary>

  - [rkbx_link for Rekordbox](#rkbx_link-for-rekordbox)
    - [Usage & Setup](#usage--setup)
    - [Supported Versions](#supported-versions-with-license)
  - [Supported protocols](#supported-protocols)
  - [Configuration](#configuration)
    - [App Settings](#app-settings)
    - [Beatkeeper](#beatkeeper-settings-for-tracking)
    - [Ableton Link](#ableton-link)
    - [OSC](#open-sound-control-osc)
    - [Track to file](.#track-to-file)
    - [Setlist to file](#setlist-to-file)
    - [sACN](#sacn)
  - [All OSC addresses](#all-osc-addresses)
  - [Troubleshooting](#troubleshooting)

</details>

## Usage & Setup
Download the latest version from [the releases](https://github.com/grufkork/rkbx_link/releases/latest). Unzip and edit the `config` file using notepad or similar:
- Set the Rekordbox version (`keeper.rekordbox_version`) you are using
- Set the correct numbers of decks (`keeper.decks`) (2 or 4)
- Enable the output modules you want to use, such as `link.enabled` or `osc.enabled`.
Then run `rkbx_link.exe` to start the program. It will automatically connect to Rekordbox and restart if it fails. During startup all available Rekordbox versions are printed.

Check the end of this document for troubleshooting tips.

Some other settings you will probably want to tune:
- `keeper.delay_compensation` to compensate for latency in your audio interface, lights or network. You can use both positive and negative values.

## Supported versions (with license)

| Rekordbox Version  |
| ----- |
| `7.2.3`, `7.2.2`, `7.1.4` |
| [`6.8.5` will be added soon] |

# Supported protocols
These are the available output modules together with what data can be sent with each. Transport export refers to sending the current beat timing, Track info is Title/Album/Artist and Phrase is the phrase analysis you can see under the waveform.
- Ableton Link (master deck transport)
- OSC (transport of any decks, phrases, track info)
- sACN (master deck transport)
- Setlist to file (logs master deck title/artist to a file and time when played)
- Track to file (stores the current track info in a file for reading in other programs)

For more details on how to configure them, check the next secion.

# Configuration
Here's in detail how to configure the app, beat tracking and output modules. The configuration is stored next to the executable in a text file named `config`.

## App settings
- `app.license <string>`
Enter your license key here to get support for the latest Rekordbox versions. Otherwise leave it empty.

- `app.auto_update <true/false>`
Enables checking for updates on startup if you have a valid [license](https://3gg.se/products/rkbx_link). 

## Beatkeeper (settings for tracking)
- `keeper.rekordbox_version <string>`
Enter the version of Rekordbox to target (eg. 6.8.5 or 7.2.2). You can see available versions on this page or when starting the program. 

- `keeper.update_rate <int>`
Number of updates per second to send. Default is 120Hz, which results in between 60Hz and 120Hz updates per second due to Windows' sleep granularity. You can set this lower if you want to save CPU usage, but it might result in less accurate timing.

- `keeper.slow_update_every_nth <int>`
How often to read non-time-critical data from Rekordbox. Saves a bit of CPU usage if increased, but will not really affect worst-case performance. Default is `10`, meaning every 10th update will read "heavier" values like the current track name and artist.

- `keeper.delay_compensation <float>`
Time in milliseconds to shift the output. Used to compensate for latency in audio, network, lights etc. Can be both negative and positive to either delay the signal or compensate for latency down the chain. If your Rekordbox audio output is before your eg. lights, increase this. If Rekordbox audio lags behind, set this to negative values.

- `keeper.keep_warm <true/false>`
Enabling this means all decks are tracked even when not active. Enabling this increases CPU usage a bit, but means that when you switch decks the new one will already be tracked and ready to go. Default is `true`. If you are outputting data from non-master decks, ensure this is on.

- `keeper.decks <int>`
Number of decks to track, 1 to 4. This decides how many decks are read from Rekordbox's memory. If you choose more decks than are active in Rekordbox, the program will fail due to trying read decks where the are not any.

## Ableton Link
- `link.enabled <true/false>`
Whether to enable Ableton Link output.

- `link.cumulative_error_tolerance <float>`
Cumulative error in beats allowed before a resync is triggered. Default is 0.05. Lower or set to zero if you really want it to track when you scratch, otherwise leave as is to save a bit of CPU and network (and to be nicer to other peers).

## Open Sound Control (OSC)
Outputs transport and more data over OSC. Check further down in this document for all addresses.
- `osc.enabled <true/false>`
Whether to enable OSC output.

- `osc.source <IP address>`
Local address to bind to. Default is 127.0.0.1:4450

- `osc.destination <IP address>`
Address to send OSC messages to. Default is 127.0.0.1:4460

- `osc.send_every_nth <int>`
Will throttle messages to only send every update_rate/send_every_nth. While tracking might run at 120Hz, OSC probably only needs to be sent at 60Hz (2) or 30Hz (4), so default of 2 is good.

- `osc.phrase_output_format <int/string/float>`
What format to send the phrase as. If int/float, it will map the phrase to an OSC int/float according to the table below. If set to string, it will send the full name of the phrase. See [DeepSymmetry Docs](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html#song-structure-tag) for more details.

| Number | Lo/Mid | Hi |
|--------|--------|--------|
| 1 | Intro | Intro 1/2 |
| 2 | Verse 1-6  | Up 1-3 |
| 3 | Chorus | Chorus 1/2 |
| 4 | Bridge | Down |
| 5 | Outro | Outro 1/2 |

### Frequent message toggles
Below are settings for toggling messages which are sent very rapidly, which might overload the receiver/channel.
- `osc.msg.beat_master <bool>`: `/beat/master`
- `osc.msg.beat_master.div_1 <bool>`: `/beat/master/div1`
- `osc.msg.beat_master.div_2 <bool>`: `/beat/master/div2`
- `osc.msg.beat_master.div_4 <bool>`: `/beat/master/div4`
- `osc.msg.time_master <bool>`: `/time/master`
- `osc.msg.phrase_master <bool>`: `/phrase/master/current`, `/phrase/master/next`, `/phrase/master/countin`

- `osc.msg.beat <bool>`: `/beat/[deck]`
- `osc.msg.beat.div_1 <bool>`: `/beat/[deck]/div1`
- `osc.msg.beat.div_2 <bool>`: `/beat/[deck]/div2`
- `osc.msg.beat.div_4 <bool>`: `/beat/[deck]/div4`
- `osc.msg.time <bool>`: `/time/[deck]`
- `osc.msg.phrase <bool>`: `/phrase/[deck]/current`, `/phrase/[deck]/next`, `/phrase/[deck]/countin`

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

## sACN
Sends the current tempo as an int on channel `start_channel` and a looping counter which increases on every beat on `start_channel+1`. Default name is "rkbx_link".
- `sacn.enabled <true/false>` Enables sACN output
- `sacn.source <x.x.x.x>` Local address to bind
- `sacn.targets <x.x.x.x,x.x.x.x,...>` Comma-separated list of target IPs
- `sacn.priority <int (1..200)>` sACN priority
- `sacn.start_channel <int (1..=511)>` 1-indexed DMX channel offset. Needs two channels to send both tempo and beats.
- `sacn.universe <int (1..=63999(` sACN universe to transmit to 
- `sacn.mode <multicast|unicast>` Default: multicast
- `sacn.source_name <string>` Max 63 ASCII chars to show as name of sender

# All OSC Addresses
`[deck]` can be `master` for the current active deck or an index (`1|2|3|4`) for a specific deck, if enabled. 
 - `/bpm/[deck]/current` (float) Current BPM of the master deck
 - `/bpm/[deck]/original` (float) Original (non-pitched) BPM of the master deck
 - `/beat/[deck]` (float) Total beat / number of beats since beat 1.1
 - `/beat/[deck]/div[1|2|4]` (float) Normalised values 0-1 looping with 1, 2 or 4 beat intervals. Good for making looping animations with 1-4 beat periods.
 - `/time/[deck]` (float) Current track position in seconds
 - `/track/[deck]/[title|artist|album]` (string) Title/artist/album of the current track on deck 1, 2, 3 or 4, or the master deck.
 - `/phrase/[deck]/current` (float/int/string depending on config) The current phrase
 - `/phrase/[deck]/next` (float/int/string) The next phrase coming up
 - `/phrase/[deck]/counting` (float) Beats until the next phrase begins.

# Troubleshooting
Try the following if you run into issues. If you even after going through all these still are having problems, please [open an issue](https://github.com/grufkork/rkbx_link/issues/new) on GitHub.

### I get "Failed to read anlz file for deck #"
This is a known issue mostly when loading tracks for the first time from streaming services. Eject and reload the track and it should be fine. The underlying cause is that the analysis file doesn't seem to yet properly exist when a non-analysed track is loaded for the first time. Should be fixable, and will probably be soon.

### The program fails to connect to Rekordbox
- Make sure you have selected the correct Rekordbox version in the config file.
- Check that have the correct number of decks set in the config file. Selecting 4 decks when you only have 2 will prevent the program from connecting.
- Ensure Rekordbox is running and has a track loaded in the deck you are trying to read.
- Try updating the program or the offsets.

### Some decks are not working
Make sure you have the correct number of decks set in the config file.

### The program starts and immediately disappears
A catastrophic failure has occurred. Open a command prompt in the directory where rkbx_link.exe is located and run `rkbx_link.exe` from there. You can now see the error in the console. You will probably want to enable debug in the config, copy the output and open an issue on GitHub.
