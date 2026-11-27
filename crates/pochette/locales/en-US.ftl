# Pochette's texts, on the desktop, in US English: the source of the translations.
# Lengths, counts and dates are in the core's own texts, in crates/pochette-core/locales.

## The main window

search-qobuz = Search Qobuz
main-menu = Main Menu
now-playing = Now Playing
tab-discover = Discover
tab-albums = Albums
menu-stats = Listening Stats
menu-sign-out = Sign Out
previews-only = This account can only play 30-second previews.

## Starting and signing in

unreachable = Can't Reach Qobuz
try-again = Try Again
sign-in-tagline = Your Qobuz library, album by album.
sign-in = Sign In with Qobuz
sign-in-hint =
    Signing in opens Qobuz in your browser.
    Requires a Qobuz subscription.
sign-in-continue = Continue in Your Browser
sign-in-continue-hint =
    Sign in to Qobuz in the browser tab that just opened.
    Pochette continues as soon as you're done.
sign-in-open-again = Open Browser Again
sign-in-copy-link = Copy Link
sign-in-cancel = Cancel
session-expired = Your session expired. Sign in again.
sign-in-cannot-save = Can't save the session: { $error }
sign-in-timed-out = The sign-in took too long. Try again.

## Settings

settings = Settings
settings-language = Language
# Follows the system's language.
settings-language-system = System Default
settings-language-restart = Pochette speaks it the next time it opens.
settings-devices = Devices

## Devices, in Settings

devices-about = Paired devices share what you play with each other, for the stats and play counts, whenever they're on the same network.
devices-paired = Paired Devices
devices-in-background = Keep syncing while Pochette is closed
devices-pair-a-device = Pair a Device
devices-pair-hint = Open Settings in Pochette on the other device as well, and it shows up here.
devices-looking = Looking for devices on this network…
devices-by-address = Pair by Address
devices-address = Its address, such as 192.168.1.20
devices-pair = Pair
devices-firewall =
    If the devices can't find each other, the network or a firewall may be in the way. Pochette needs TCP port { $port } to sync, and UDP port 5353 to find devices.
devices-pair-with = Pair With { $name }
devices-cancel = Cancel
devices-paired-with = Paired with { $name }.
devices-not-paired = Not paired with { $name }.
devices-declined = { $name } declined to pair.
devices-pairing-failed = Can't pair with { $name }: { $reason }
devices-syncing = Syncing…
# $when says how long ago, as "3 minutes ago".
devices-synced = Synced { $when }
devices-synced-error = Synced { $when }. { $error }
devices-not-synced = Not synced yet
devices-forget = Forget
devices-forget-tooltip = Stop syncing with this device
devices-cannot-change = Can't change it: { $error }
devices-connecting = Connecting…
devices-confirm = Check that { $name } shows the same code, then pair on both.
devices-waiting = Waiting for { $name } to confirm…
sync-no-bus = Syncing needs a D-Bus session bus: { $error }
sync-cannot-start = Can't start syncing: { $error }
sync-no-answer = it didn't answer

## Everywhere

back = Back
play = Play
pause = Pause
previous = Previous
next = Next
see-all = See All
favorite-add = Add to Your Albums
favorite-remove = Remove From Your Albums
genres = Genres
genres-all = All Genres
decades = Decades
decades-all = All Decades
# A decade, by its first year: "1970s".
decade = { $decade }s
# How the albums are sorted: $sort is "Date Added", $order "Newest First".
sort-by = Sort By { $sort }, { $order }
sort-picked = { $sort }, { $order }

## The Albums tab

albums-cannot-load = Can't Load Your Albums
albums-none = No Saved Albums
albums-none-hint = Albums you add to your favorites on Qobuz appear here.
albums-search = Search albums, artists, labels
albums-random = Play a Random Album
albums-random-filtered = Play a Random Album From These
albums-no-match = No Matching Albums
albums-clear-filters = Clear Search and Filters

## The Discover tab

discover-cannot-load = Can't Load Discover
discover-section-cannot-load = Can't Load These Albums
discover-section-empty = No Albums Here
discover-new-releases = New Releases
discover-press-awards = Press Awards
discover-ideal-discography = Ideal Discography

## Searching Qobuz

# The search, as the page's title.
search-query = “{ $query }”
search-failed = Can't Search Qobuz
search-nothing-found = Nothing Found
search-no-albums = No Albums Found
search-artists = Artists

## An album

album-cannot-load = Can't load the tracks: { $error }
album-about = About This Album
# The share button: it copies the album's link, to paste it anywhere.
album-copy-link = Copy Link
album-link-copied = Link Copied
# $plays is "12 plays", $last when, as "yesterday".
album-played = { $plays } · last played { $last }
hi-res = Hi-Res
album-disc = Disc { $number }
track-unavailable = Not available for streaming

## A track, from a link: Pochette plays albums, whole, so it offers the track's album

track-heading = A Track on Its Own?
track-albums-only = Pochette believes songs belong with their albums, in the order the artist chose. Taking one out would be like reading a single chapter of a novel.
# $track is the track's title, $album its album's.
track-on-album = “{ $track }” is waiting for you on { $album }, right where the artist put it.
track-open-album = Open the Album
track-cannot-load = Can't find the album this track is on: { $error }

## An artist

# Above the artist's name, in capitals.
artist-kicker = Artist
artist-more = More
artist-less = Less
artist-cannot-load = Can't load this artist: { $error }
artist-no-albums = No albums to stream from this artist.
artist-similar = Similar Artists
releases-albums = Albums
releases-eps-singles = EPs & Singles
releases-live = Live
releases-compilations = Compilations
releases-other = Other Releases
releases-cannot-load = Can't Load These Albums
releases-empty = No Albums Here

## Now playing

now-playing-quality = Quality
now-playing-show-cover = Show the Cover
now-playing-show-lyrics = Show the Lyrics
now-playing-position = Position
now-playing-volume = Volume
now-playing-tracks = Tracks
output = Output
output-default = System Default
quality-best = Best Quality
quality-hint = Each track streams in the best it has, up to this, as your subscription allows.
quality-mp3 = MP3 · 320 kbps
quality-cd = CD · 16-bit / 44.1 kHz
quality-hi-res-96 = Hi-Res · up to 24-bit / 96 kHz
quality-hi-res-192 = Hi-Res · up to 24-bit / 192 kHz
# The quality of the track playing, when it's an MP3.
quality-mp3-playing = MP3 · 320 kbps
playback-cannot-start = Can't start playback: { $error }
playback-cannot-stream = Can't stream “{ $title }”: { $error }
lyrics-failed = Can't load the lyrics
lyrics-none = No lyrics for this track
lyrics-instrumental = Instrumental

## Listening stats

stats-year = Year
stats-month = Month
stats-all-time = All Time
stats-whole-year = Whole Year
stats-top-albums = Top Albums
stats-top-artists = Top Artists
stats-top-genres = Top Genres
stats-nothing = Nothing Played Yet
stats-nothing-hint = Play some albums: how much you listen, your top albums, artists and genres show up here.
stats-listened = Listened
stats-album-plays = Album Plays
stats-artists = Artists
stats-by-year = By Year
stats-by-month = By Month
stats-by-day = By Day
# A day of a month: $month is its name, $number its number, from 1.
stats-day = { $month } { $day }
# A bar's tooltip: "March: 12 h".
stats-bar = { $name }: { $time }
# A genre's share of the time listened.
stats-share = { $percent }%
